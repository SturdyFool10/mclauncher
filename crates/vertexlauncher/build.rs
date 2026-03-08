fn main() {
    #[cfg(target_os = "windows")]
    {
        use image::ImageFormat;
        use std::path::PathBuf;

        println!("cargo:rerun-if-changed=../launcher_ui/src/assets/vertex.webp");

        let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is not set"));
        let icon_path = out_dir.join("vertex.ico");
        let decoded = image::load_from_memory_with_format(
            include_bytes!("../launcher_ui/src/assets/vertex.webp"),
            ImageFormat::WebP,
        )
        .expect("failed to decode vertex.webp for Windows icon");
        decoded
            .save_with_format(&icon_path, ImageFormat::Ico)
            .expect("failed to write generated .ico icon");

        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(icon_path.to_string_lossy().as_ref());
        resource
            .compile()
            .expect("failed to compile Windows resources");
    }
}
