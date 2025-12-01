use crate::SqliteGraphError;

pub fn parse_required_i64(args: &[String], flag: &str) -> Result<i64, SqliteGraphError> {
    let value = required_value(args, flag)?;
    value
        .parse::<i64>()
        .map_err(|_| invalid(format!("{flag} expects an integer")))
}

pub fn parse_optional_u32(args: &[String], flag: &str) -> Option<u32> {
    value(args, flag)?.parse::<u32>().ok()
}

pub fn required_value(args: &[String], flag: &str) -> Result<String, SqliteGraphError> {
    value(args, flag).ok_or_else(|| invalid(format!("missing {flag}")))
}

pub fn value(args: &[String], flag: &str) -> Option<String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            return iter.next().cloned();
        }
    }
    None
}

pub fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

pub fn encode(
    object: serde_json::Map<String, serde_json::Value>,
) -> Result<String, SqliteGraphError> {
    const ERR_PREFIX: &str = "cli";
    serde_json::to_string(&serde_json::Value::Object(object))
        .map_err(|e| invalid(format!("{ERR_PREFIX} serialization failed: {e}")))
}

pub fn invalid<T: Into<String>>(message: T) -> SqliteGraphError {
    SqliteGraphError::invalid_input(message.into())
}
