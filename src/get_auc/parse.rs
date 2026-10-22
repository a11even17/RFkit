use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::error::Error;
use std::io::{ErrorKind, Read};
use std::str;
pub struct Q1Q3RtI {
    pub q1: f32,
    pub q3: f32,
    pub rt_i_l: Vec<(f32, f32)>,
}
pub fn mzml(mzml_f: &std::path::Path) -> Result<Vec<Q1Q3RtI>, Box<dyn Error>> {
    let mut reader = Reader::from_file(mzml_f)?;
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<Vec<u8>>::new();
    let mut buf = Vec::new();
    let mut rt_l = Vec::<f32>::new();
    let mut i_l = Vec::<f32>::new();
    let mut buf0 = Vec::<u8>::new();
    let mut buf1 = Vec::<u8>::new();
    let mut zlibc = None;
    let mut pre64 = None;
    let mut time_arr = None;
    let mut time_in_seconds = false;
    let mut q1 = f32::NAN;
    let mut q3 = f32::NAN;
    let mut q1q3eics = Vec::new();
    let mut saw_run = false;
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Start(e) => {
                if e.local_name().as_ref() == b"run" {
                    saw_run = true;
                }
                if e.local_name().as_ref() == b"chromatogram" {
                    rt_l.clear();
                    i_l.clear();
                    q1 = f32::NAN;
                    q3 = f32::NAN;
                }
                if e.local_name().as_ref() == b"binaryDataArray" {
                    zlibc = None;
                    pre64 = None;
                    time_arr = None;
                    time_in_seconds = false;
                }
                stack.push(e.local_name().as_ref().to_vec());
            }
            Event::End(e) => {
                if e.local_name().as_ref() == b"chromatogram" {
                    if q1.is_finite() && q3.is_finite() {
                        if rt_l.is_empty() || rt_l.len() != i_l.len() {
                            return Err("chromatogram time and intensity arrays must be nonempty and have equal lengths".into());
                        }
                        if rt_l.iter().chain(&i_l).any(|value| !value.is_finite())
                            || rt_l.windows(2).any(|pair| pair[0] > pair[1])
                        {
                            return Err(
                                "chromatogram values must be finite and retention times sorted"
                                    .into(),
                            );
                        }
                        q1q3eics.push(Q1Q3RtI {
                            q1,
                            q3,
                            rt_i_l: rt_l.drain(..).zip(i_l.drain(..)).collect(),
                        });
                    }
                    q1 = f32::NAN;
                    q3 = f32::NAN;
                }
                stack.pop();
            }
            Event::Empty(e) if e.local_name().as_ref() == b"cvParam" => {
                let Ok(Some(accession)) = e.try_get_attribute("accession") else {
                    continue;
                };
                match accession.value.as_ref() {
                    b"MS:1000827" => match stack.iter().rev().nth(1).map(Vec::as_slice) {
                        Some(b"precursor") => {
                            if let Ok(Some(value)) = e.try_get_attribute("value") {
                                q1 = str::from_utf8(&value.value)?.parse()?;
                            }
                        }
                        Some(b"product") => {
                            if let Ok(Some(value)) = e.try_get_attribute("value") {
                                q3 = str::from_utf8(&value.value)?.parse()?;
                            }
                        }
                        _ => {}
                    },
                    b"MS:1000523" => pre64 = Some(true),
                    b"MS:1000521" => pre64 = Some(false),
                    b"MS:1000574" => zlibc = Some(true),
                    b"MS:1000576" => zlibc = Some(false),
                    b"MS:1000595" => {
                        time_arr = Some(true);
                        time_in_seconds = e
                            .try_get_attribute("unitAccession")?
                            .is_some_and(|unit| unit.value.as_ref() == b"UO:0000010");
                    }
                    b"MS:1000515" => time_arr = Some(false),
                    _ => {}
                }
            }
            Event::Text(e)
                if stack.last().is_some_and(|name| name == b"binary")
                    && stack.iter().any(|name| name == b"chromatogram") =>
            {
                let Some(time_arr) = time_arr else {
                    continue;
                };
                let pre64 = pre64.ok_or("chromatogram floating point precision not set")?;
                let arr_l = if time_arr { &mut rt_l } else { &mut i_l };
                decode_bin(
                    &e.decode()?,
                    zlibc.ok_or("chromatogram compression type not set")?,
                    pre64,
                    arr_l,
                    &mut buf0,
                    &mut buf1,
                )?;
                if time_arr && time_in_seconds {
                    for time in arr_l {
                        *time /= 60.0;
                    }
                }
            }
            _ => (),
        }
        buf.clear();
    }
    if !saw_run || !stack.is_empty() {
        return Err("mzML is missing a complete run element".into());
    }
    q1q3eics.sort_unstable_by(|x, y| x.q1.total_cmp(&y.q1));
    Ok(q1q3eics)
}
fn decode_bin(
    bin: &str,
    zlibc: bool,
    pre64: bool,
    arr_l: &mut Vec<f32>,
    buf0: &mut Vec<u8>,
    buf1: &mut Vec<u8>,
) -> std::io::Result<()> {
    let mut wrapped_reader = bin.as_bytes();
    let mut decoder = base64::read::DecoderReader::new(
        &mut wrapped_reader,
        &base64::engine::general_purpose::STANDARD,
    );
    buf0.clear();
    decoder.read_to_end(buf0)?;
    let buf2 = if zlibc {
        buf1.clear();
        flate2::read::ZlibDecoder::new(buf0.as_slice()).read_to_end(buf1)?;
        buf1
    } else {
        buf0
    };
    arr_l.clear();
    let element_size = if pre64 {
        std::mem::size_of::<f64>()
    } else {
        std::mem::size_of::<f32>()
    };
    if buf2.len() % element_size != 0 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "binary array contains an incomplete floating point value",
        ));
    }
    if pre64 {
        arr_l.extend(
            buf2.chunks_exact(std::mem::size_of::<f64>())
                .map(|s| f64::from_le_bytes(s.try_into().unwrap()) as f32),
        );
    } else {
        arr_l.extend(
            buf2.chunks_exact(std::mem::size_of::<f32>())
                .map(|s| f32::from_le_bytes(s.try_into().unwrap())),
        );
    }
    Ok(())
}
