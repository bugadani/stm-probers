use std::{borrow::Cow, fmt::Write as _, time::Instant};

use probe_rs_target::{ChipFamily, MemoryAccess, MemoryRegion, NvmRegion, RamRegion};
use quick_xml::Reader;
use stm32_data_gen::memory::{self, Memory};
use stm32_data_serde::chip::memory::Kind;

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
            )
        })
        .chain([(
            "STM32H7RS",
            String::from("sources/embassy/devices/STM32H7.db"),
            String::from("sources/probe-rs/probe-rs/targets/STM32H7RS_Series.yaml"),
            String::from("output/STM32H7RS_Series.yaml"),
        )]);

    _ = std::fs::create_dir("output");

    let start = Instant::now();
    for (family_name, variants_xml, probe_rs_data, output) in families {
        println!("Processing {family_name}");
        let family = family_members(&variants_xml);

        let yaml = std::fs::read_to_string(&probe_rs_data).unwrap();
        let mut family_data = serde_yaml::from_str::<ChipFamily>(&yaml).unwrap();

        for device in family.devices {
            let Some(mut memories) = memory::get(&device.device) else {
                println!("Missing embassy data for {}", device.device);
                continue;
            };
            memories.sort_by(|a, b| a.address.cmp(&b.address));

            if no_package_variants.contains(&family_name) {
                update_variant(&mut family_data, &device.device, &memories);
            } else {
                for (_, variant) in device.chip_variants() {
                    update_variant(&mut family_data, &variant, &memories);
                }
            }
        }

        let yaml = serialize_to_yaml_string(&family_data);
        std::fs::write(&output, yaml)
            .unwrap_or_else(|e| panic!("Failed to write to {output}: {e}"));
    }
    let end = start.elapsed();
    println!("Finished in {:.02}s", end.as_secs_f32());
}

fn update_variant(family_data: &mut ChipFamily, variant: &str, memories: &[Memory]) {
    let Some(var) = family_data.variants.iter_mut().find(|v| v.name == variant) else {
        println!("Missing from probe-rs: {variant}");
        return;
    };
    var.memory_map.clear();
    let cores = var
        .cores
        .iter()
        .map(|core| core.name.clone())
        .collect::<Vec<_>>();

    for mem in memories.iter().filter(|m| m.kind == Kind::Flash) {
        let start = mem.address as u64;
        let size = mem.size as u64;
        let range = start..start + size;
        var.memory_map.push(MemoryRegion::Nvm(NvmRegion {
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
        }));
    }

    for mem in memories.iter().filter(|m| m.kind == Kind::Ram) {
        let start = mem.address as u64;
        let size = mem.size as u64;
        let range = start..start + size;

        let access_by_core = match (variant, mem.name.as_str()) {
            // Skip SRAM2 because by default its inaccessible by the main core
            (n, "SRAM2A" | "SRAM2B") if n.starts_with("STM32WB") => continue,
            (n, "SRAM2A_ICODE" | "SRAM2B_ICODE") if n.starts_with("STM32WB") => continue,
            // Allow all cores by default
            _ => cores.clone(),
        };

        let access_attrs = match (variant, mem.name.as_str()) {
            (n, "CCMRAM") if n.starts_with("STM32F4") => MemoryAccess {
                read: true,
                write: true,
                execute: false,
                boot: false,
            },
            _ => MemoryAccess {
                read: true,
                write: true,
                execute: true,
                boot: false,
            },
        };

        var.memory_map.push(MemoryRegion::Ram(RamRegion {
            name: Some(mem.name.clone()),
            range,
            access: Some(access_attrs),
            cores: access_by_core,
        }));
    }
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

/// Some optimizations to improve the readability of the `serde_yaml` output:
/// - If `Option<T>` is `None`, it is serialized as `null` ... we want to omit it.
/// - If `Vec<T>` is empty, it is serialized as `[]` ... we want to omit it.
/// - `serde_yaml` serializes hex formatted integers as single quoted strings, e.g. '0x1234' ... we need to remove the single quotes so that it round-trips properly.
pub fn serialize_to_yaml_string(family: &ChipFamily) -> String {
    let raw_yaml_string = serde_yaml::to_string(family).unwrap();

    let mut yaml_string = String::with_capacity(raw_yaml_string.len());
    for reader_line in raw_yaml_string.lines() {
        let trimmed_line = reader_line.trim();
        if reader_line.ends_with(": null")
            || reader_line.ends_with(": []")
            || reader_line.ends_with(": {}")
            || reader_line.ends_with(": false")
        {
            // Some fields have default-looking, but significant values that we want to keep.
            let keep_default = [
                "rtt_scan_ranges: []",
                "read: false",
                "write: false",
                "execute: false",
            ];
            if !keep_default.contains(&trimmed_line) {
                // Skip the line
                continue;
            }
        } else {
            // Some fields have different default values than the type may indicate.
            let trim_nondefault = ["read: true", "write: true", "execute: true"];
            if trim_nondefault.contains(&trimmed_line) {
                // Skip the line
                continue;
            }
        }

        let mut reader_line = Cow::Borrowed(reader_line);
        if (reader_line.contains("'0x") || reader_line.contains("'0X"))
            && reader_line.ends_with('\'')
        {
            // Remove the single quotes
            reader_line = reader_line.replace('\'', "").into();
        }

        yaml_string.write_str(&reader_line).unwrap();
        yaml_string.push('\n');
    }

    // Second pass: remove empty `access:` objects
    let mut output = String::with_capacity(yaml_string.len());
    let mut lines = yaml_string.lines().peekable();
    while let Some(line) = lines.next() {
        if line.trim() == "access:" {
            let indent_level = line.find(|c: char| c != ' ').unwrap_or(0);

            let Some(next) = lines.peek() else {
                // No other lines, access is empty, skip it
                continue;
            };

            let next_indent_level = next.find(|c: char| c != ' ').unwrap_or(0);
            if next_indent_level <= indent_level {
                // Access is empty, skip it
                continue;
            }
        }

        output.push_str(line);
        output.push('\n');
    }

    output
}
