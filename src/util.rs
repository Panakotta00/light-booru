pub fn sanitize_tag(tag: &str) -> String {
    tag.trim().replace(" ", "_").replace("\t", "_").to_lowercase()
}