use std::error::Error;
use std::path::PathBuf;

pub struct Param {
    pub t_list: PathBuf,
    pub mz_tol: f32,
    pub offset: f32,
}

pub fn read_param() -> Result<Param, Box<dyn Error>> {
    let param = std::fs::read_to_string("param.txt")?;
    let value = param.parse::<toml::Table>()?;
    let transition_list = value
        .get("transition_list")
        .and_then(toml::Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .ok_or("transition_list must be a nonempty path in param.txt")?;
    let offset = value
        .get("offset")
        .and_then(|value| {
            value
                .as_float()
                .or_else(|| value.as_integer().map(|value| value as f64))
        })
        .ok_or("offset must be a number in param.txt")? as f32;
    if !offset.is_finite() {
        return Err("offset must be finite in param.txt".into());
    }
    Ok(Param {
        t_list: PathBuf::from(transition_list),
        mz_tol: 0.06,
        offset,
    })
}
