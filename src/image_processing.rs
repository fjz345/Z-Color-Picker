#[repr(C)]
#[derive(Clone, Debug)]
pub struct Rgb {
    pub val: (u8, u8, u8),
}

#[derive(Debug)]
pub struct FramePixelRead {
    pub width: usize,
    pub height: usize,
    pub data: Vec<Rgb>,
}

pub fn u8u8u8u8_to_u8(buf: &[(u8, u8, u8, u8)]) -> Vec<u8> {
    let mut ret: Vec<u8> = Vec::new();
    for i in 0..buf.len() {
        ret.push(buf[i].0);
        ret.push(buf[i].1);
        ret.push(buf[i].2);
        ret.push(buf[i].3);
    }

    ret
}

pub fn u8u8u8_to_u8u8u8u8(buf: &[Rgb]) -> Vec<(u8, u8, u8, u8)> {
    let mut ret: Vec<(u8, u8, u8, u8)> = Vec::new();
    for i in 0..buf.len() {
        ret.push((buf[i].val.0, buf[i].val.1, buf[i].val.2, 255));
    }

    ret
}
