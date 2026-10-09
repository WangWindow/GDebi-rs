use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    let languages = fs::read_to_string("po/LINGUAS")?;
    let root = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));

    for language in languages
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let po_path = format!("po/{language}.po");
        println!("cargo:rerun-if-changed={po_path}");
        let source = fs::read_to_string(&po_path)?;
        let messages = parse_po(&source);
        let out_dir = root.join("locale").join(language).join("LC_MESSAGES");
        fs::create_dir_all(&out_dir)?;
        write_mo(&out_dir.join("gdebi-rs.mo"), &messages)?;
    }

    println!("cargo:rerun-if-changed=po/LINGUAS");
    Ok(())
}

fn parse_po(source: &str) -> BTreeMap<String, String> {
    let mut messages = BTreeMap::new();
    let mut msgid: Option<String> = None;
    let mut msgstr: Option<String> = None;
    let mut reading_msgstr = false;

    let finish = |messages: &mut BTreeMap<String, String>,
                  msgid: &mut Option<String>,
                  msgstr: &mut Option<String>| {
        if let (Some(id), Some(value)) = (msgid.take(), msgstr.take()) {
            if !id.is_empty() && !value.is_empty() {
                messages.insert(id, value);
            }
        } else {
            msgid.take();
            msgstr.take();
        }
    };

    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() {
            finish(&mut messages, &mut msgid, &mut msgstr);
            reading_msgstr = false;
        } else if let Some(value) = line.strip_prefix("msgid ") {
            finish(&mut messages, &mut msgid, &mut msgstr);
            msgid = Some(parse_po_string(value));
            reading_msgstr = false;
        } else if let Some(value) = line.strip_prefix("msgstr ") {
            msgstr = Some(parse_po_string(value));
            reading_msgstr = true;
        } else if line.starts_with('"') {
            let value = parse_po_string(line);
            if reading_msgstr {
                msgstr.get_or_insert_with(String::new).push_str(&value);
            } else {
                msgid.get_or_insert_with(String::new).push_str(&value);
            }
        }
    }
    finish(&mut messages, &mut msgid, &mut msgstr);
    messages
}

fn parse_po_string(value: &str) -> String {
    let quoted = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value);
    let mut result = String::new();
    let mut escaped = false;
    for character in quoted.chars() {
        if escaped {
            result.push(match character {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '\\' => '\\',
                '"' => '"',
                other => other,
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            result.push(character);
        }
    }
    if escaped {
        result.push('\\');
    }
    result
}

fn write_mo(path: &std::path::Path, messages: &BTreeMap<String, String>) -> io::Result<()> {
    let entries: Vec<(&str, &str)> = messages
        .iter()
        .map(|(message, translation)| (message.as_str(), translation.as_str()))
        .collect();
    let count = entries.len() as u32;
    let original_table = 28u32;
    let translation_table = original_table + count * 8;
    let original_data = translation_table + count * 8;
    let original_data_len: u32 = entries.iter().map(|(id, _)| id.len() as u32 + 1).sum();
    let translation_data = original_data + original_data_len;

    let mut bytes = Vec::new();
    write_u32(&mut bytes, 0x9504_12de);
    write_u32(&mut bytes, 0);
    write_u32(&mut bytes, count);
    write_u32(&mut bytes, original_table);
    write_u32(&mut bytes, translation_table);
    write_u32(&mut bytes, 0);
    write_u32(&mut bytes, 0);

    let mut offset = original_data;
    for (id, _) in &entries {
        write_u32(&mut bytes, id.len() as u32);
        write_u32(&mut bytes, offset);
        offset += id.len() as u32 + 1;
    }

    offset = translation_data;
    for (_, translation) in &entries {
        write_u32(&mut bytes, translation.len() as u32);
        write_u32(&mut bytes, offset);
        offset += translation.len() as u32 + 1;
    }

    for (id, _) in &entries {
        bytes.extend_from_slice(id.as_bytes());
        bytes.push(0);
    }
    for (_, translation) in &entries {
        bytes.extend_from_slice(translation.as_bytes());
        bytes.push(0);
    }

    fs::write(path, bytes)
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
