fn main() {
    #[cfg(target_os = "windows")]
    {
        use image::{ImageReader, codecs::ico::IcoEncoder, imageops::FilterType};
        use std::fs::File;
        use std::io::Cursor;
        use std::path::PathBuf;

        println!("cargo:rerun-if-changed=../launcher_ui/src/assets/vertex.webp");

        let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is not set"));
        let icon_path = out_dir.join("vertex.ico");
        let decoded = ImageReader::new(Cursor::new(include_bytes!(
            "../launcher_ui/src/assets/vertex.webp"
        )))
        .with_guessed_format()
        .expect("failed to detect vertex icon format")
        .decode()
        .expect("failed to decode vertex icon source image");
        let resized = decoded.resize(256, 256, FilterType::Lanczos3).to_rgba8();
        let mut icon_file = File::create(&icon_path).expect("failed to create generated .ico icon");
        IcoEncoder::new(&mut icon_file)
            .encode(
                resized.as_raw(),
                resized.width(),
                resized.height(),
                image::ExtendedColorType::Rgba8,
            )
            .expect("failed to write generated .ico icon");

        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(icon_path.to_string_lossy().as_ref());
        resource
            .compile()
            .expect("failed to compile Windows resources");
    }
}
