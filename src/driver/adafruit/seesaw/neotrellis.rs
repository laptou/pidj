pub const fn neo_trellis_key(x: u8) -> u8 {
    ((x) / 4) * 8 + ((x) % 4)
}

pub const fn neo_trellis_seesaw_key(x: u8) -> u8 {
    ((x) / 8) * 4 + ((x) % 8)
}
