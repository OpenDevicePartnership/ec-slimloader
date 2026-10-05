pub fn protected_if<const TRUE: u32, R, E: FnOnce() -> u32, B: FnOnce() -> R>(
    expression: E,
    body: B,
) -> Result<R, u32> {
    let expression_value = expression();
    if expression_value == TRUE {
        Ok(body())
    } else {
        Err(expression_value)
    }
}
