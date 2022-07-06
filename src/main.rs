use clap::{App, SubCommand, Arg};
use mdbook::{preprocess::{Preprocessor, CmdPreprocessor}, errors::Error};
use mdbook_ansi::Ansi;

fn main() -> Result<(), Error> {
    let matches = App::new("mdbook-ansi")
        .about("A preprocessor that renders ANSI expansions in fenced code blocks.")
        .subcommand(
            SubCommand::with_name("supports")
                .arg(Arg::with_name("renderer").required(true))
                .about("Check whether a renderer is supported by this preprocessor"),
        )
        .get_matches();

    let pre = Ansi;

    if let Some(sub_args) = matches.subcommand_matches("supports") {
        let renderer = sub_args.value_of("renderer").expect("Required argument");
        let supported = pre.supports_renderer(renderer);
        if supported {
            Ok(())
        } else {
            Err(Error::msg(format!(
                "The katex preprocessor does not support the '{}' renderer",
                &renderer
            )))
        }
    } else {
        let (ctx, book) = CmdPreprocessor::parse_input(std::io::stdin())?;

        if ctx.mdbook_version != mdbook::MDBOOK_VERSION {
            eprintln!(
                "Warning: The mdbook-ansi preprocessor was built against version \
                 {} of mdbook, but we're being called from version {}",
                mdbook::MDBOOK_VERSION,
                ctx.mdbook_version
            );
        }

        let processed_book = Ansi.run(&ctx, book)?;
        serde_json::to_writer(std::io::stdout(), &processed_book)?;

        Ok(())
    }
}