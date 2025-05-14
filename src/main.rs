use std::{collections::HashMap, time::Instant};

use probe_rs_target::{ChipFamily, MemoryAccess, MemoryRegion, NvmRegion, RamRegion};
use stm32_data_serde::{
    chip::{
        memory::{Access, Kind},
        Memory,
    },
    Chip,
};
use target_gen::commands::elf::serialize_to_yaml_string;

fn main() {
    let embassy_data = load_stm_data("sources/stm32-data-generated/data/chips/");

    //for (family, chips) in &embassy_data {
    //    println!("  {family}: {}", chips.len());
    //}

    _ = std::fs::create_dir("output");

    let mut unknown_variants = Vec::new();
    let start = Instant::now();
    for (family_name, chips) in embassy_data.iter() {
        let probe_rs_data = format!("sources/probe-rs/probe-rs/targets/{family_name}_Series.yaml");
        let output = format!("output/{family_name}_Series.yaml");

        println!("Processing {family_name}");

        let yaml = std::fs::read_to_string(&probe_rs_data).unwrap();
        let mut family_data = serde_yaml::from_str::<ChipFamily>(&yaml).unwrap();

        for device in chips.iter() {
            let mut memories = device.memory[0].clone(); // TODO: support multi-bank?
            memories.sort_by(|a, b| a.address.cmp(&b.address));

            update_variant(&mut family_data, &device.name, &device.name, &memories);

            add_package_variants(
                &mut family_data,
                device
                    .packages
                    .iter()
                    .map(|variant| (device.name.as_str(), variant.name.clone())),
            );
        }

        deduplicate_package_variants(&mut family_data);
        deduplicate_by_name(&mut family_data);

        unknown_variants.extend(extra_variants(&family_data, &chips));

        let yaml = serialize_to_yaml_string(&family_data).unwrap();
        std::fs::write(&output, yaml)
            .unwrap_or_else(|e| panic!("Failed to write to {output}: {e}"));
    }

    for variant in unknown_variants {
        println!("embassy data is missing {variant}");
    }

    let end = start.elapsed();
    println!("Finished in {:.02}s", end.as_secs_f32());
}

fn extra_variants<'a>(
    family_data: &'a ChipFamily,
    chips: &'a [Chip],
) -> impl Iterator<Item = String> + 'a {
    family_data
        .variants()
        .iter()
        .flat_map(|chip| chip.package_variants())
        .filter(|chip| {
            !chips
                .iter()
                .any(|c| &c.name == *chip || c.packages.iter().any(|p| &p.name == *chip))
        })
        .map(|chip| chip.clone())
}

fn load_stm_data(arg: &str) -> HashMap<String, Vec<Chip>> {
    let stm32_data = std::fs::read_dir(arg).unwrap();

    let start = Instant::now();

    let mut families = HashMap::<String, Vec<Chip>>::new();

    for entry in stm32_data {
        let entry = entry.unwrap();

        if entry.file_name().to_string_lossy().ends_with(".json") {
            let contents = std::fs::read_to_string(entry.path()).unwrap();

            let chip = serde_json::from_str::<Chip>(&contents)
                .unwrap_or_else(|e| panic!("Failed to parse JSON: {e}"));

            let family = match chip.family.as_str() {
                "STM32L4+" => "STM32L4".to_string(),
                "STM32H7" if chip.name.starts_with("STM32H7R") => "STM32H7RS".to_string(),
                "STM32H7" if chip.name.starts_with("STM32H7S") => "STM32H7RS".to_string(),
                other => other.to_string(),
            };

            families.entry(family).or_default().push(chip);
        }
    }

    println!(
        "Loaded {} families in {}s",
        families.len(),
        start.elapsed().as_secs_f32()
    );

    families
}

fn deduplicate_package_variants(family_data: &mut ChipFamily) {
    for variant in &mut family_data.variants {
        let mut seen = std::collections::HashSet::new();
        variant.package_variants.retain(|v| seen.insert(v.clone()));
    }
}

fn add_package_variants<'a>(
    family_data: &mut ChipFamily,
    chip_variants: impl Iterator<Item = (&'a str, String)>,
) {
    for (device, package) in chip_variants {
        // Look up device in family
        let Some(variant) = family_data.variants.iter_mut().find(|v| v.name == device) else {
            println!("Missing from probe-rs: {package}");
            continue;
        };

        // Add package variant
        variant.package_variants.push(package);
    }
}

fn deduplicate_by_name(family_data: &mut ChipFamily) {
    let mut seen = std::collections::HashSet::new();
    family_data.variants.retain(|v| seen.insert(v.name.clone()));
}

fn update_variant(
    family_data: &mut ChipFamily,
    variant: &str,
    out_name: &str,
    memories: &[Memory],
) {
    let Some(var) = family_data.variants.iter_mut().find(|v| v.name == variant) else {
        println!("Missing from probe-rs: {variant}");
        return;
    };

    // Rename variant to the package-less format
    var.name = out_name.to_string();

    var.memory_map.clear();
    let cores = var
        .cores
        .iter()
        .map(|core| core.name.clone())
        .collect::<Vec<_>>();

    for mem in memories {
        let start = mem.address as u64;
        let size = mem.size as u64;
        let range = start..start + size;

        let access = mem.access.unwrap_or_else(|| Access {
            read: true,
            write: matches!(mem.kind, Kind::Ram),
            execute: true,
        });
        let access_attrs = MemoryAccess {
            read: access.read,
            write: access.write,
            execute: access.execute,
            boot: matches!(mem.kind, Kind::Flash),
        };

        let region = match mem.kind {
            Kind::Flash if mem.name == "OTP" => MemoryRegion::Nvm(NvmRegion {
                name: Some(mem.name.clone()),
                range,
                access: Some(MemoryAccess {
                    read: true,
                    write: false,
                    execute: false,
                    boot: false,
                }),
                cores: cores.clone(),
                is_alias: false,
            }),
            Kind::Flash => MemoryRegion::Nvm(NvmRegion {
                name: Some(mem.name.clone()),
                range,
                access: Some(MemoryAccess {
                    read: true,
                    write: false,
                    execute: true,
                    boot: true,
                }),
                cores: cores.clone(),
                is_alias: false,
            }),
            Kind::Ram => {
                let access_by_core = match (variant, mem.name.as_str()) {
                    // Skip SRAM2 because by default its inaccessible by the main core
                    (n, "SRAM2A" | "SRAM2B") if n.starts_with("STM32WB") => continue,
                    (n, "SRAM2A_ICODE" | "SRAM2B_ICODE") if n.starts_with("STM32WB") => continue,
                    // Allow all cores by default
                    _ => cores.clone(),
                };

                MemoryRegion::Ram(RamRegion {
                    name: Some(mem.name.clone()),
                    range,
                    access: Some(access_attrs),
                    cores: access_by_core,
                })
            }
            Kind::Eeprom => continue, // TODO
        };
        var.memory_map.push(region);
    }

    merge_consecutive_flash_regions(&mut var.memory_map);
}

fn merge_consecutive_flash_regions(memory_map: &mut Vec<MemoryRegion>) {
    let mut iter = memory_map.iter().peekable();

    let mut output = Vec::new();
    while let Some(region) = iter.next() {
        let region = region.clone();

        let MemoryRegion::Nvm(mut region) = region else {
            output.push(region);
            continue;
        };
        let Some(name) = region.name.clone() else {
            output.push(MemoryRegion::Nvm(region));
            continue;
        };

        let Some((bank, _)) = name.split_once("_REGION_") else {
            output.push(MemoryRegion::Nvm(region));
            continue;
        };

        region.name = Some(bank.to_string());

        while let Some(next) = iter.peek() {
            let MemoryRegion::Nvm(next) = next else {
                break;
            };

            if !next.name.as_deref().unwrap_or_default().starts_with(bank) {
                break;
            }

            if region.range.end != next.range.start {
                break;
            }

            region.range.end = next.range.end;
            iter.next();
        }

        output.push(MemoryRegion::Nvm(region));
    }

    *memory_map = output;
}
