pub(crate) struct Options {
    pub(crate) crash: bool,
    pub(crate) url: String,
}

pub(crate) fn options() -> Result<Options, &'static str> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("--approve") {
        return Err("Simulated refunds only. Usage: refund --approve [--crash] [sqlite-url]");
    }
    let first = args.next();
    let crash = first.as_deref() == Some("--crash");
    let url =
        if crash { args.next() } else { first }.unwrap_or_else(|| "sqlite://refund-demo.db".into());
    if args.next().is_some() {
        return Err("Unexpected argument");
    }
    Ok(Options { crash, url })
}
