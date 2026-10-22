use std::error::Error;
use std::io::{BufRead, BufReader};
type AcqTime = (u8, u8, f32, f32);
type SequenceTimes = Vec<(u8, Vec<AcqTime>)>;
pub fn read(param_t: &crate::common::Param) -> Result<(), Box<dyn Error>> {
    let rt_l = read_log()?;
    write_rt(&rt_l, param_t.offset)
}
fn write_rt(rt_l: &[(u8, Vec<AcqTime>)], offset: f32) -> Result<(), Box<dyn Error>> {
    let tf = |x: f32| -> String { format!("{:.3}", x / 60. + offset) };
    let mut wtr = csv::WriterBuilder::new().from_path("acq_time.csv")?;
    for (seqn, rt_mzml) in rt_l {
        wtr.write_record([&format!("sequence{seqn}.mzML"), "", ""])?;
        for (row, col, sta, end) in rt_mzml {
            wtr.write_record([format!("({row}, {col})"), tf(*sta), tf(*end)])?;
        }
    }
    wtr.flush()?;
    Ok(())
}
fn read_log() -> Result<SequenceTimes, Box<dyn Error>> {
    let reader = BufReader::new(std::fs::File::open("batch.rftime")?);
    let mut start_end = Vec::new();
    let mut seq = None;
    let mut pending = None;
    for (line_number, line) in reader.lines().enumerate().skip(2) {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let line_sp: Vec<&str> = line.trim_end().split('\t').collect();
        if line_sp.len() < 6 {
            return Err(format!(
                "batch.rftime line {} requires six tab-separated columns",
                line_number + 1
            )
            .into());
        }
        let sequence = line_sp[2].parse::<u8>()?;
        if Some(sequence) != seq {
            if pending.is_some() {
                return Err("batch.rftime sequence changed before the well's end row".into());
            }
            seq = Some(sequence);
            start_end.push((sequence, Vec::new()));
        }
        let time = line_sp[5].parse::<f32>()?;
        if !time.is_finite() {
            return Err("batch.rftime times must be finite".into());
        }
        if line_sp[3] == "3001" {
            let (row, col, start) = pending
                .take()
                .ok_or("batch.rftime contains an end row without a start row")?;
            if time < start {
                return Err("batch.rftime well end time precedes its start time".into());
            }
            start_end
                .last_mut()
                .unwrap()
                .1
                .push((row, col, start, time));
        } else {
            if pending.is_some() {
                return Err("batch.rftime contains a well without an end row".into());
            }
            pending = Some((line_sp[3].parse()?, line_sp[4].parse()?, time));
        }
    }
    if pending.is_some() || start_end.is_empty() {
        return Err("batch.rftime contains no complete acquisition rows".into());
    }
    Ok(start_end)
}
