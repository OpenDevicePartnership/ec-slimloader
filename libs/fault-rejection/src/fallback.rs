pub fn protected_if<const TRUE: u32, R, E: FnOnce() -> u32, B: FnOnce() -> R>(expression: E, body: B) -> Option<R> {
    if expression() == TRUE { Some(body()) } else { None }
}
