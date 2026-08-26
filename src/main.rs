#[actix_web::main]
async fn main() -> std::io::Result<()> {
    racebin::run().await
}
