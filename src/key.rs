use regex::Regex;
use std::{collections::BTreeMap, sync::LazyLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Key {
    pub root: i8,
    pub accidental: i8,
    pub scale: i32,
}

#[hooq::hooq(anyhow)]
pub fn parse(name: &str, scales: &BTreeMap<String, i32>) -> anyhow::Result<Option<Key>> {
    static TAG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\A\[key=([A-G])([#b]?):([^\]\r\n]+)\]").expect("valid key tag pattern")
    });
    let Some(start) = name.find("[key=") else {
        return Ok(None);
    };
    let captures = TAG
        .captures(&name[start..])
        .ok_or_else(|| anyhow::anyhow!("Invalid key tag: {name}"))?;
    let mut fifths: i8 = match &captures[1] {
        "C" => 0,
        "D" => 2,
        "E" => 4,
        "F" => -1,
        "G" => 1,
        "A" => 3,
        "B" => 5,
        _ => unreachable!("note validated by regex"),
    };
    fifths += match &captures[2] {
        "#" => 7,
        "b" => -7,
        _ => 0,
    };
    let scale = *scales
        .get(&captures[3])
        .ok_or_else(|| anyhow::anyhow!("Unknown scale: {}", &captures[3]))?;
    let root = (fifths * 7).rem_euclid(12);
    let signature_offset = match scale {
        0xAD5 => 1,
        0x6AD => -2,
        0x5AB => -4,
        0x6B5 => -1,
        0x5AD | 0x9AD | 0xAAD | 0x4A9 | 0x4E9 => -3,
        0x56B => -5,
        _ => 0,
    };
    fifths += signature_offset;
    if matches!(scale, 0xAB5 | 0x5AD) && !(-7..=7).contains(&fifths) {
        anyhow::bail!("Key signature out of range: {name}");
    }
    Ok(Some(Key {
        root,
        accidental: match &captures[2] {
            "#" => 1,
            "b" => -1,
            _ => fifths.signum(),
        },
        scale,
    }))
}
