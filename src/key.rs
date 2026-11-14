use regex::Regex;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Key {
    pub root: i8,
    pub accidental: i8,
    pub scale: i32,
}

#[hooq::hooq(anyhow)]
pub fn parse(name: &str) -> anyhow::Result<Option<Key>> {
    static TAG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\A\[key=([A-G])([#b]?):(major|minor)\]").expect("valid key tag pattern")
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
    let minor = i8::from(&captures[3] == "minor");
    fifths -= minor * 3;
    if !(-7..=7).contains(&fifths) {
        anyhow::bail!("Key signature out of range: {name}");
    }
    Ok(Some(Key {
        root: (fifths * 7 + minor * 9).rem_euclid(12),
        accidental: fifths.signum(),
        scale: if minor == 1 { 0x5AD } else { 0xAB5 },
    }))
}
