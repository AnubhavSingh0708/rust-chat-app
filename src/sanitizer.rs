use ammonia::Builder;

pub fn sanitize_chat_message(input: &str) -> String {
    // Prevent restricted CSS styles
    let lower_input = input.to_lowercase();
    if lower_input.contains("display:") || 
       lower_input.contains("opacity: 0") || 
       lower_input.contains("opacity:0") {
        return "<i>[Blocked by Security Filter]</i>".to_string();
    }

    let mut builder = Builder::empty();
    // Allow ONLY <span> tags and the "style" attribute on spans
    builder.add_tags(["span"])
           .add_tag_attributes("span", ["style"]);

    // Any other tags (like <script>, <b>, <img>) will have their tags stripped, 
    // leaving only the safe text behind.
    builder.clean(input).to_string()
}