//! 敏感输入解码只返回固定错误，避免 serde 将提交的秘密放进错误链。
use anyhow::{Result, anyhow, bail};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub(crate) fn safe_from_value<T: DeserializeOwned>(value: Value, label: &str) -> Result<T> {
    serde_json::from_value(value).map_err(|_| anyhow!("{label} 格式无效"))
}

pub(crate) fn advanced_fields(text: &str) -> Result<Map<String, Value>> {
    let value: Value = serde_json::from_str(text)
        .map_err(|_| anyhow!("高级配置必须是有效的 JSON 对象；输入内容未被写入。"))?;
    let Value::Object(fields) = value else {
        bail!("高级配置必须是 JSON 对象。");
    };
    if fields
        .keys()
        .any(|key| !["args", "env", "headers", "secret"].contains(&key.as_str()))
    {
        bail!("高级配置仅支持 args、env、headers、secret。");
    }
    for (key, value) in &fields {
        match key.as_str() {
            "args" => {
                safe_from_value::<Vec<String>>(value.clone(), "高级配置 args")?;
            }
            "secret" => {
                safe_from_value::<String>(value.clone(), "高级配置 secret")?;
            }
            "env" => {
                safe_from_value::<BTreeMap<String, String>>(value.clone(), "高级配置 env")?;
            }
            "headers" => {
                safe_from_value::<BTreeMap<String, String>>(value.clone(), "高级配置 headers")?;
            }
            _ => unreachable!(),
        }
    }
    Ok(fields)
}
