use std::io::Read;
fn main() {
    let result = (|| -> anyhow::Result<()> {
        let mut input = Vec::new();
        std::io::stdin().take(1024 * 1024).read_to_end(&mut input)?;
        let request: media_engine::ImageRequest = serde_json::from_slice(&input)?;
        let result = media_engine::image_work::execute(request)?;
        println!("{}", serde_json::to_string(&result)?);
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
