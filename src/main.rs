use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::Instant,
};

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
    let mut embassy_data = load_stm_data("sources/stm32-data-generated/data/chips/");

    //for (family, chips) in &embassy_data {
    //    println!("  {family}: {}", chips.len());
    //}

    _ = std::fs::create_dir("output");

    let mut unknown_variants = Vec::new();
    let start = Instant::now();
    for (family_name, embassy_chips) in embassy_data.iter_mut() {
        let probe_rs_data = format!("sources/probe-rs/probe-rs/targets/{family_name}_Series.yaml");
        let output = format!("output/{family_name}_Series.yaml");

        if !Path::new(&probe_rs_data).exists() {
            println!("Skipping {family_name} as probe-rs data does not exist");
            continue;
        }

        println!("Processing {family_name}");

        let yaml = std::fs::read_to_string(&probe_rs_data).unwrap();
        let mut family_data = serde_yaml::from_str::<ChipFamily>(&yaml).unwrap();

        preprocess_family(&mut family_data, embassy_chips);

        for embassy_device in embassy_chips.iter() {
            let mut memories = embassy_device.memory[0].clone(); // TODO: support multi-bank?
            memories.sort_by(|a, b| a.address.cmp(&b.address));

            update_variant(
                &mut family_data,
                &embassy_device.name,
                &embassy_device.name,
                &memories,
            );

            add_package_variants(
                &mut family_data,
                &embassy_device.name,
                embassy_device
                    .packages
                    .iter()
                    .map(|variant| variant.name.clone()),
            );
        }

        deduplicate_package_variants(&mut family_data);

        unknown_variants.extend(remove_unknown_variants(&mut family_data, &embassy_chips));

        let yaml = serialize_to_yaml_string(&family_data).unwrap();
        std::fs::write(&output, yaml)
            .unwrap_or_else(|e| panic!("Failed to write to {output}: {e}"));
    }

    if !unknown_variants.is_empty() {
        println!("The following chip variants were not found in embassy-stm32 data and have been deleted from probe-rs:");
        for variant in unknown_variants {
            println!(" - {variant}");
        }
    }

    let end = start.elapsed();
    println!("Finished in {:.02}s", end.as_secs_f32());
}

fn preprocess_family(family_data: &mut ChipFamily, embassy_chips: &mut Vec<Chip>) {
    // Normalize chip names. probe-rs data may contain one-variant-per-chip data when freshly
    // generated from a pack, or many-variants-per-chip data when updating already processed data.
    // This function essentially erases variant data that hasn't been processed by this tool.
    for chip in family_data.variants.iter_mut() {
        // We don't care about packaging.
        let chip_name = chip.name.trim_end_matches("TR");

        // Find the embassy chip where the chip or variant name equals chip_name:
        let Some(embassy_chip) = embassy_chip_from_variant(embassy_chips, chip_name) else {
            println!("embassy data is missing {chip_name}");
            continue;
        };
        chip.name = embassy_chip.name.clone();
    }

    // Deduplicate probe-rs chips
    let mut seen = HashSet::new();
    family_data
        .variants
        .retain(|chip| seen.insert(chip.name.clone()));

    // Don't bother processing embassy chips that don't have probe-rs data. These need to be added via a CMSIS-Pack update.
    embassy_chips.retain(|chip| family_data.variants.iter().any(|v| v.name == chip.name));
}

fn embassy_chip_from_variant<'a>(embassy_chips: &'a [Chip], chip_name: &str) -> Option<&'a Chip> {
    embassy_chips
        .iter()
        .find(|c| &c.name == chip_name || c.packages.iter().any(|p| p.name == chip_name))
}

fn remove_unknown_variants(family_data: &mut ChipFamily, chips: &[Chip]) -> Vec<String> {
    let mut extras = vec![];

    for chip in family_data.variants.iter_mut() {
        chip.package_variants.retain(|chip| {
            let exists = embassy_chip_from_variant(chips, chip).is_some();

            if !exists {
                extras.push(chip.clone());
            }

            exists
        });
    }

    extras
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

fn add_package_variants(
    family_data: &mut ChipFamily,
    device: &str,
    chip_variants: impl Iterator<Item = String>,
) {
    for package in chip_variants {
        // Look up device in family
        let Some(variant) = family_data.variants.iter_mut().find(|v| v.name == device) else {
            println!("Missing from probe-rs: {device}");
            continue;
        };

        // Add package variant
        variant.package_variants.push(package);
    }
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
