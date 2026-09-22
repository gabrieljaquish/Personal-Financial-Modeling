//! A small Mach-O reader: just enough structure for the release pipeline to say
//! **where** two builds of the executable differ (`PLAN.md` §4.1, front-end build
//! determinism and the `lipo` comparison), and to check what `dist` must hold
//! (the deployment target, the absence of a `cargo-auditable` section, the
//! architectures).
//!
//! It understands fat (universal) files and 64-bit little-endian thin files —
//! the only two shapes `cargo build` and `lipo` produce for the two macOS
//! targets — and splits a thin file into named byte ranges: the header and load
//! commands, every section with file contents, and each blob of `__LINKEDIT`
//! that a load command locates (symbol table, string table, code signature,
//! fixups, function starts, …). Whatever a load command does not locate is
//! reported as the unattributed remainder of its segment. No external tool is
//! involved, so the report is the same on every machine.

use std::fmt::Write as _;

const FAT_MAGIC: u32 = 0xcafe_babe;
const FAT_MAGIC_64: u32 = 0xcafe_babf;
const MH_MAGIC_64: u32 = 0xfeed_facf;

const LC_SYMTAB: u32 = 0x2;
const LC_DYSYMTAB: u32 = 0xb;
const LC_SEGMENT_64: u32 = 0x19;
const LC_UUID: u32 = 0x1b;
const LC_CODE_SIGNATURE: u32 = 0x1d;
const LC_FUNCTION_STARTS: u32 = 0x26;
const LC_DATA_IN_CODE: u32 = 0x29;
const LC_BUILD_VERSION: u32 = 0x32;
const LC_DYLD_INFO_ONLY: u32 = 0x8000_0022;
const LC_DYLD_EXPORTS_TRIE: u32 = 0x8000_0033;
const LC_DYLD_CHAINED_FIXUPS: u32 = 0x8000_0034;

/// `CPU_TYPE_ARM64` and `CPU_TYPE_X86_64`.
const CPU_ARM64: u32 = 0x0100_000c;
const CPU_X86_64: u32 = 0x0100_0007;

/// A named byte range of a thin Mach-O, relative to the start of the slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Region {
    pub(crate) name: String,
    pub(crate) offset: usize,
    pub(crate) size: usize,
}

/// One architecture's image.
#[derive(Debug, Clone)]
pub(crate) struct Thin {
    pub(crate) arch: String,
    /// Offset of this slice in the file (0 for a thin file).
    pub(crate) file_offset: usize,
    pub(crate) size: usize,
    pub(crate) regions: Vec<Region>,
    pub(crate) uuid: Option<String>,
    /// `LC_BUILD_VERSION` minimum OS and SDK, as `major.minor[.patch]`.
    pub(crate) minos: Option<String>,
    pub(crate) sdk: Option<String>,
    pub(crate) has_code_signature: bool,
    /// Every `(segment, section)` pair, for presence checks.
    pub(crate) sections: Vec<(String, String)>,
}

/// A parsed file: one slice for a thin file, several for a fat one.
#[derive(Debug, Clone)]
pub(crate) struct MachO {
    pub(crate) fat: bool,
    pub(crate) slices: Vec<Thin>,
}

impl MachO {
    /// The architectures in slice order (`arm64`, `x86_64`).
    pub(crate) fn arches(&self) -> Vec<String> {
        self.slices.iter().map(|s| s.arch.clone()).collect()
    }
}

/// Whether `bytes` start like a Mach-O this module reads.
pub(crate) fn is_macho(bytes: &[u8]) -> bool {
    matches!(be32(bytes, 0), Some(FAT_MAGIC | FAT_MAGIC_64)) || le32(bytes, 0) == Some(MH_MAGIC_64)
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_be_bytes(s.try_into().expect("four bytes")))
}
fn be64(b: &[u8], at: usize) -> Option<u64> {
    b.get(at..at + 8)
        .map(|s| u64::from_be_bytes(s.try_into().expect("eight bytes")))
}
fn le32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes(s.try_into().expect("four bytes")))
}
fn le64(b: &[u8], at: usize) -> Option<u64> {
    b.get(at..at + 8)
        .map(|s| u64::from_le_bytes(s.try_into().expect("eight bytes")))
}

fn name16(b: &[u8], at: usize) -> String {
    let raw = b.get(at..at + 16).unwrap_or(&[]);
    let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

fn arch_name(cputype: u32) -> String {
    match cputype {
        CPU_ARM64 => "arm64".to_owned(),
        CPU_X86_64 => "x86_64".to_owned(),
        other => format!("cputype-{other:#x}"),
    }
}

fn usize_of(v: u64) -> Result<usize, String> {
    usize::try_from(v).map_err(|_| format!("offset {v} does not fit in usize"))
}

/// Parses a fat or thin Mach-O.
pub(crate) fn parse(bytes: &[u8]) -> Result<MachO, String> {
    match be32(bytes, 0) {
        Some(magic @ (FAT_MAGIC | FAT_MAGIC_64)) => {
            let wide = magic == FAT_MAGIC_64;
            let n = be32(bytes, 4).ok_or("truncated fat header")? as usize;
            let stride = if wide { 32 } else { 20 };
            let mut slices = Vec::with_capacity(n);
            for i in 0..n {
                let at = 8 + i * stride;
                let cputype = be32(bytes, at).ok_or("truncated fat_arch")?;
                let (offset, size) = if wide {
                    (
                        usize_of(be64(bytes, at + 8).ok_or("truncated fat_arch_64")?)?,
                        usize_of(be64(bytes, at + 16).ok_or("truncated fat_arch_64")?)?,
                    )
                } else {
                    (
                        be32(bytes, at + 8).ok_or("truncated fat_arch")? as usize,
                        be32(bytes, at + 12).ok_or("truncated fat_arch")? as usize,
                    )
                };
                let slice = bytes
                    .get(offset..offset + size)
                    .ok_or("fat slice lies outside the file")?;
                let mut thin = parse_thin(slice)?;
                if thin.arch != arch_name(cputype) {
                    return Err("fat_arch cputype disagrees with the slice header".to_owned());
                }
                thin.file_offset = offset;
                slices.push(thin);
            }
            Ok(MachO { fat: true, slices })
        }
        _ => Ok(MachO {
            fat: false,
            slices: vec![parse_thin(bytes)?],
        }),
    }
}

fn version(v: u32) -> String {
    let (major, minor, patch) = (v >> 16, (v >> 8) & 0xff, v & 0xff);
    if patch == 0 {
        format!("{major}.{minor}")
    } else {
        format!("{major}.{minor}.{patch}")
    }
}

#[allow(clippy::too_many_lines)]
fn parse_thin(b: &[u8]) -> Result<Thin, String> {
    if le32(b, 0) != Some(MH_MAGIC_64) {
        return Err("not a 64-bit little-endian Mach-O".to_owned());
    }
    let cputype = le32(b, 4).ok_or("truncated header")?;
    let ncmds = le32(b, 16).ok_or("truncated header")? as usize;
    let sizeofcmds = le32(b, 20).ok_or("truncated header")? as usize;
    let header_end = 32 + sizeofcmds;
    if header_end > b.len() {
        return Err("load commands extend past the file".to_owned());
    }
    let mut regions = vec![Region {
        name: "header + load commands".to_owned(),
        offset: 0,
        size: header_end,
    }];
    let mut segments: Vec<Region> = Vec::new();
    let mut located: Vec<Region> = Vec::new();
    let mut sections = Vec::new();
    let mut uuid = None;
    let mut minos = None;
    let mut sdk = None;
    let mut has_code_signature = false;

    let mut at = 32;
    for _ in 0..ncmds {
        let cmd = le32(b, at).ok_or("truncated load command")?;
        let size = le32(b, at + 4).ok_or("truncated load command")? as usize;
        if size < 8 || at + size > header_end {
            return Err("malformed load command size".to_owned());
        }
        let mut blob = |name: &str, off_at: usize, size_at: usize| -> Result<(), String> {
            let off = le32(b, at + off_at).ok_or("truncated load command")? as usize;
            let len = le32(b, at + size_at).ok_or("truncated load command")? as usize;
            if len > 0 {
                located.push(Region {
                    name: format!("__LINKEDIT {name}"),
                    offset: off,
                    size: len,
                });
            }
            Ok(())
        };
        match cmd {
            LC_SEGMENT_64 => {
                let segname = name16(b, at + 8);
                let fileoff = usize_of(le64(b, at + 40).ok_or("truncated segment")?)?;
                let filesize = usize_of(le64(b, at + 48).ok_or("truncated segment")?)?;
                let nsects = le32(b, at + 64).ok_or("truncated segment")? as usize;
                segments.push(Region {
                    name: segname.clone(),
                    offset: fileoff,
                    size: filesize,
                });
                for s in 0..nsects {
                    let sa = at + 72 + s * 80;
                    let sectname = name16(b, sa);
                    let seg = name16(b, sa + 16);
                    let sect_size = usize_of(le64(b, sa + 40).ok_or("truncated section")?)?;
                    let sect_off = le32(b, sa + 48).ok_or("truncated section")? as usize;
                    let flags = le32(b, sa + 64).ok_or("truncated section")?;
                    sections.push((seg.clone(), sectname.clone()));
                    // Zero-fill sections (S_ZEROFILL 0x1, S_GB_ZEROFILL 0xc,
                    // S_THREAD_LOCAL_ZEROFILL 0x12) occupy no file bytes.
                    let zerofill = matches!(flags & 0xff, 0x1 | 0xc | 0x12);
                    if !zerofill && sect_off != 0 && sect_size != 0 {
                        located.push(Region {
                            name: format!("{seg},{sectname}"),
                            offset: sect_off,
                            size: sect_size,
                        });
                    }
                }
            }
            LC_SYMTAB => {
                let symoff = le32(b, at + 8).ok_or("truncated symtab")? as usize;
                let nsyms = le32(b, at + 12).ok_or("truncated symtab")? as usize;
                let stroff = le32(b, at + 16).ok_or("truncated symtab")? as usize;
                let strsize = le32(b, at + 20).ok_or("truncated symtab")? as usize;
                if nsyms > 0 {
                    located.push(Region {
                        name: "__LINKEDIT symbol table".to_owned(),
                        offset: symoff,
                        size: nsyms * 16,
                    });
                }
                if strsize > 0 {
                    located.push(Region {
                        name: "__LINKEDIT string table".to_owned(),
                        offset: stroff,
                        size: strsize,
                    });
                }
            }
            LC_DYSYMTAB => {
                let indirect_off = le32(b, at + 56).ok_or("truncated dysymtab")? as usize;
                let n = le32(b, at + 60).ok_or("truncated dysymtab")? as usize;
                if n > 0 {
                    located.push(Region {
                        name: "__LINKEDIT indirect symbols".to_owned(),
                        offset: indirect_off,
                        size: n * 4,
                    });
                }
            }
            LC_CODE_SIGNATURE => {
                has_code_signature = true;
                blob("code signature", 8, 12)?;
            }
            LC_FUNCTION_STARTS => blob("function starts", 8, 12)?,
            LC_DATA_IN_CODE => blob("data in code", 8, 12)?,
            LC_DYLD_EXPORTS_TRIE => blob("exports trie", 8, 12)?,
            LC_DYLD_CHAINED_FIXUPS => blob("chained fixups", 8, 12)?,
            LC_DYLD_INFO_ONLY => {
                for (i, name) in ["rebase", "bind", "weak bind", "lazy bind", "export"]
                    .iter()
                    .enumerate()
                {
                    blob(&format!("dyld info: {name}"), 8 + i * 8, 12 + i * 8)?;
                }
            }
            LC_UUID => {
                let raw = b.get(at + 8..at + 24).ok_or("truncated uuid")?;
                let mut text = String::new();
                for byte in raw {
                    let _ = write!(text, "{byte:02x}");
                }
                uuid = Some(text);
            }
            LC_BUILD_VERSION => {
                minos = le32(b, at + 12).map(version);
                sdk = le32(b, at + 16).map(version);
            }
            _ => {}
        }
        at += size;
    }

    for region in &located {
        if region.offset + region.size > b.len() {
            return Err(format!("{} extends past the file", region.name));
        }
    }
    // What a segment holds that no load command above locates (alignment
    // padding, a blob of a command this reader does not know) is reported as
    // the segment's remainder, so no byte escapes the report.
    for seg in &segments {
        if seg.size == 0 {
            continue;
        }
        let end = seg.offset + seg.size;
        if end > b.len() {
            return Err(format!("segment {} extends past the file", seg.name));
        }
        let mut inside: Vec<&Region> = located
            .iter()
            .filter(|r| r.offset >= seg.offset && r.offset + r.size <= end)
            .collect();
        if seg.offset == 0 {
            inside.push(&regions[0]);
        }
        inside.sort_by_key(|r| r.offset);
        let mut cursor = seg.offset;
        let mut gaps = Vec::new();
        for r in inside {
            if r.offset > cursor {
                gaps.push((cursor, r.offset));
            }
            cursor = cursor.max(r.offset + r.size);
        }
        if end > cursor {
            gaps.push((cursor, end));
        }
        for (n, (from, to)) in gaps.into_iter().enumerate() {
            regions.push(Region {
                name: format!("{} unattributed bytes #{}", seg.name, n + 1),
                offset: from,
                size: to - from,
            });
        }
    }
    regions.extend(located);

    Ok(Thin {
        arch: arch_name(cputype),
        file_offset: 0,
        size: b.len(),
        regions,
        uuid,
        minos,
        sdk,
        has_code_signature,
        sections,
    })
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
pub(crate) mod tests {
    use super::*;

    /// A minimal, synthetic thin arm64 Mach-O: one `__TEXT` segment holding one
    /// `__text` section, a `__LINKEDIT` segment with a string table, a UUID and a
    /// build-version command. Only the fields this reader looks at are filled in.
    pub(crate) fn synthetic(text: &[u8], strings: &[u8], uuid: [u8; 16]) -> Vec<u8> {
        let mut cmds = Vec::new();
        // LC_SEGMENT_64 __TEXT with one section.
        let text_off = 0x400_u64;
        let mut seg = Vec::new();
        seg.extend(LC_SEGMENT_64.to_le_bytes());
        seg.extend(((72 + 80) as u32).to_le_bytes());
        let mut name = [0_u8; 16];
        name[..6].copy_from_slice(b"__TEXT");
        seg.extend(name);
        seg.extend(0_u64.to_le_bytes()); // vmaddr
        seg.extend(0x1000_u64.to_le_bytes()); // vmsize
        seg.extend(0_u64.to_le_bytes()); // fileoff
        seg.extend((text_off + text.len() as u64).to_le_bytes()); // filesize
        seg.extend(5_u32.to_le_bytes());
        seg.extend(5_u32.to_le_bytes());
        seg.extend(1_u32.to_le_bytes()); // nsects
        seg.extend(0_u32.to_le_bytes());
        let mut sect = [0_u8; 16];
        sect[..6].copy_from_slice(b"__text");
        seg.extend(sect);
        seg.extend(name);
        seg.extend(0_u64.to_le_bytes()); // addr
        seg.extend((text.len() as u64).to_le_bytes()); // size
        seg.extend((text_off as u32).to_le_bytes()); // offset
        seg.extend([0_u8; 4 * 7]); // align, reloff, nreloc, flags, reserved1-3
        cmds.extend(seg);
        // LC_SEGMENT_64 __LINKEDIT, no sections.
        let link_off = text_off + text.len() as u64;
        let mut seg = Vec::new();
        seg.extend(LC_SEGMENT_64.to_le_bytes());
        seg.extend(72_u32.to_le_bytes());
        let mut name = [0_u8; 16];
        name[..10].copy_from_slice(b"__LINKEDIT");
        seg.extend(name);
        seg.extend(0x1000_u64.to_le_bytes());
        seg.extend(0x1000_u64.to_le_bytes());
        seg.extend(link_off.to_le_bytes());
        seg.extend((strings.len() as u64).to_le_bytes());
        seg.extend([0_u8; 16]);
        cmds.extend(seg);
        // LC_SYMTAB: no symbols, the string table.
        cmds.extend(LC_SYMTAB.to_le_bytes());
        cmds.extend(24_u32.to_le_bytes());
        cmds.extend(0_u32.to_le_bytes());
        cmds.extend(0_u32.to_le_bytes());
        cmds.extend((link_off as u32).to_le_bytes());
        cmds.extend((strings.len() as u32).to_le_bytes());
        // LC_UUID.
        cmds.extend(LC_UUID.to_le_bytes());
        cmds.extend(24_u32.to_le_bytes());
        cmds.extend(uuid);
        // LC_BUILD_VERSION: macOS, minos 12.0, sdk 15.2, no tools.
        cmds.extend(LC_BUILD_VERSION.to_le_bytes());
        cmds.extend(24_u32.to_le_bytes());
        cmds.extend(1_u32.to_le_bytes());
        cmds.extend((12_u32 << 16).to_le_bytes());
        cmds.extend(((15_u32 << 16) | (2 << 8)).to_le_bytes());
        cmds.extend(0_u32.to_le_bytes());

        let mut out = Vec::new();
        out.extend(MH_MAGIC_64.to_le_bytes());
        out.extend(CPU_ARM64.to_le_bytes());
        out.extend(0_u32.to_le_bytes()); // cpusubtype
        out.extend(2_u32.to_le_bytes()); // MH_EXECUTE
        out.extend(5_u32.to_le_bytes()); // ncmds
        out.extend((cmds.len() as u32).to_le_bytes());
        out.extend(0_u32.to_le_bytes()); // flags
        out.extend(0_u32.to_le_bytes()); // reserved
        out.extend(cmds);
        out.resize(text_off as usize, 0);
        out.extend(text);
        out.extend(strings);
        out
    }

    #[test]
    fn reads_the_synthetic_image() {
        let bytes = synthetic(b"CODECODE", b"\0_main\0", [0xab; 16]);
        assert!(is_macho(&bytes));
        let m = parse(&bytes).unwrap();
        assert!(!m.fat);
        let t = &m.slices[0];
        assert_eq!(t.arch, "arm64");
        assert_eq!(t.minos.as_deref(), Some("12.0"));
        assert_eq!(t.sdk.as_deref(), Some("15.2"));
        assert_eq!(t.uuid.as_deref(), Some("abababababababababababababababab"));
        assert!(!t.has_code_signature);
        assert!(t
            .sections
            .contains(&("__TEXT".to_owned(), "__text".to_owned())));
        let names: Vec<&str> = t.regions.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"__TEXT,__text"), "{names:?}");
        assert!(names.contains(&"__LINKEDIT string table"), "{names:?}");
        // The padding between the load commands and __text is attributed.
        assert!(names.contains(&"__TEXT unattributed bytes #1"), "{names:?}");
        let text = t
            .regions
            .iter()
            .find(|r| r.name == "__TEXT,__text")
            .unwrap();
        assert_eq!(&bytes[text.offset..text.offset + text.size], b"CODECODE");
    }

    #[test]
    fn reads_a_fat_wrapper() {
        let thin = synthetic(b"CODE", b"\0", [1; 16]);
        let mut fat = Vec::new();
        fat.extend(FAT_MAGIC.to_be_bytes());
        fat.extend(1_u32.to_be_bytes());
        fat.extend(CPU_ARM64.to_be_bytes());
        fat.extend(0_u32.to_be_bytes());
        fat.extend(0x1000_u32.to_be_bytes());
        fat.extend((thin.len() as u32).to_be_bytes());
        fat.extend(14_u32.to_be_bytes());
        fat.resize(0x1000, 0);
        fat.extend(&thin);
        let m = parse(&fat).unwrap();
        assert!(m.fat);
        assert_eq!(m.arches(), ["arm64"]);
        assert_eq!(m.slices[0].file_offset, 0x1000);
    }

    #[test]
    fn rejects_what_it_cannot_read() {
        assert!(!is_macho(b"\x7fELF...."));
        assert!(parse(b"not a mach-o at all").is_err());
        let mut truncated = synthetic(b"CODE", b"\0", [1; 16]);
        truncated.truncate(40);
        assert!(parse(&truncated).is_err());
    }
}
