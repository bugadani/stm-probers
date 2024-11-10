use std::time::Instant;

use probe_rs_target::{ChipFamily, MemoryAccess, MemoryRegion, NvmRegion, RamRegion};
use quick_xml::Reader;
use stm32_data_gen::memory::{self, Access, Memory};
use stm32_data_serde::chip::memory::Kind;
use target_gen::commands::elf::serialize_to_yaml_string;

fn is_prefixed_with_any(s: &str, prefixes: Option<impl AsRef<[&'static str]>>) -> Option<bool> {
    let Some(prefixes) = prefixes else {
        return None;
    };
    Some(prefixes.as_ref().iter().any(|prefix| s.starts_with(prefix)))
}

fn main() {
    let families = [
        "STM32C0", "STM32F0", "STM32F1", "STM32F2", "STM32F3", "STM32F4", "STM32F7", "STM32G0",
        "STM32G4", "STM32H5", "STM32H7", "STM32L0", "STM32L1", "STM32L4", "STM32L5", "STM32U0",
        "STM32U5", "STM32WB", "STM32WBA", "STM32WL",
    ];
    let no_package_variants = ["STM32F1"];
    let families = families
        .iter()
        .map(|f| {
            (
                *f,
                format!("sources/embassy/devices/{f}.db"),
                format!("sources/probe-rs/probe-rs/targets/{f}_Series.yaml"),
                format!("output/{f}_Series.yaml"),
                None,
                None,
            )
        })
        .chain([(
            "STM32H7",
            String::from("sources/embassy/devices/STM32H7.db"),
            String::from("sources/probe-rs/probe-rs/targets/STM32H7_Series.yaml"),
            String::from("output/STM32H7_Series.yaml"),
            None,
            Some(vec!["STM32H7R", "STM32H7S"]),
        )])
        .chain([(
            "STM32H7RS",
            String::from("sources/embassy/devices/STM32H7.db"),
            String::from("sources/probe-rs/probe-rs/targets/STM32H7RS_Series.yaml"),
            String::from("output/STM32H7RS_Series.yaml"),
            Some(vec!["STM32H7R", "STM32H7S"]),
            None,
        )]);

    _ = std::fs::create_dir("output");

    let start = Instant::now();
    for (family_name, variants_xml, probe_rs_data, output, allow_prefix, reject_prefix) in families
    {
        println!("Processing {family_name}");
        let family = family_members(&variants_xml);

        let yaml = std::fs::read_to_string(&probe_rs_data).unwrap();
        let mut family_data = serde_yaml::from_str::<ChipFamily>(&yaml).unwrap();

        for device in family.devices {
            if is_prefixed_with_any(&device.device, allow_prefix.as_ref()) == Some(false) {
                continue;
            }
            if is_prefixed_with_any(&device.device, reject_prefix.as_ref()) == Some(true) {
                continue;
            }
            let Some(mut memories) = memory::get(&device.device) else {
                println!("Missing embassy data for {}", device.device);
                continue;
            };
            memories.sort_by(|a, b| a.address.cmp(&b.address));

            if no_package_variants.contains(&family_name) {
                update_variant(&mut family_data, &device.device, &device.device, &memories);
            } else {
                for (_, variant) in device.chip_variants() {
                    update_variant(&mut family_data, &variant, &device.device, &memories);
                }
            }

            add_package_variants(&mut family_data, device.chip_variants());
        }

        deduplicate_by_name(&mut family_data);

        let yaml = serialize_to_yaml_string(&family_data).unwrap();
        std::fs::write(&output, yaml)
            .unwrap_or_else(|e| panic!("Failed to write to {output}: {e}"));
    }
    let end = start.elapsed();
    println!("Finished in {:.02}s", end.as_secs_f32());
}

fn add_package_variants<'a>(
    family_data: &mut ChipFamily,
    chip_variants: impl Iterator<Item = (&'a str, String)>,
) {
    for (device, package) in chip_variants {
        // Look up device in family
        let Some(variant) = family_data.variants.iter_mut().find(|v| v.name == device) else {
            println!("Missing from probe-rs: {device}");
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

#[derive(Default)]
struct Family {
    name: String,
    devices: Vec<Device>,
}

#[derive(Default)]
struct Device {
    pn: String,
    device: String,
    variants: Vec<String>,
}

impl Device {
    fn chip_variants(&self) -> impl Iterator<Item = (&str, String)> + '_ {
        self.variants.iter().map(|variant| {
            if self.pn.contains('-') {
                (self.device.as_str(), self.pn.replace('-', variant))
            } else {
                (self.device.as_str(), format!("{}{}", self.pn, variant))
            }
        })
    }
}

fn family_members(family: &str) -> Family {
    let file =
        std::fs::read_to_string(family).unwrap_or_else(|e| panic!("Failed to read {family}: {e}"));
    let mut reader = Reader::from_str(&file);
    reader.trim_text(true);

    let mut qpath = String::with_capacity(100);

    let mut current_device = Device::default();
    let mut family = Family::default();

    loop {
        match reader.read_event().unwrap() {
            quick_xml::events::Event::Start(tag) => {
                let tag_name = String::from_utf8_lossy(tag.name().into_inner());
                if !qpath.is_empty() {
                    qpath.push('.');
                }
                qpath.push_str(&*tag_name);
            }
            quick_xml::events::Event::End(tag) => {
                match qpath.as_str() {
                    "family.subFamily.device" => {
                        family.devices.push(std::mem::take(&mut current_device));
                    }
                    _ => {}
                }

                let tag_name = String::from_utf8_lossy(tag.name().into_inner());
                qpath = if let Some((rest, expected)) = qpath.rsplit_once('.') {
                    assert!(tag_name == expected);
                    rest.to_string()
                } else {
                    assert!(tag_name == qpath);
                    qpath.clear();
                    qpath
                };
            }
            quick_xml::events::Event::Empty(_) => {}
            quick_xml::events::Event::Text(text) => {
                let text = String::from_utf8_lossy(&text.into_inner()).to_string();

                match qpath.as_str() {
                    "family" => family.name = text,
                    "family.subFamily.device.PN" => {
                        let pieces = text.split(',').collect::<Vec<_>>();
                        current_device.pn = pieces[0].to_string();
                        current_device.device = pieces[pieces.len() - 1].to_string();
                    }
                    "family.subFamily.device.variants" => {
                        current_device.variants =
                            text.split(',').map(String::from).collect::<Vec<_>>()
                    }
                    _ => {}
                }
            }
            quick_xml::events::Event::Eof => break,
            quick_xml::events::Event::Comment(_) => {}
            other => println!("{:?}", other),
        }
    }

    family
}
