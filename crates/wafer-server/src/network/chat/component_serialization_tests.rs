use super::component_serialization::{decode, decode_or_partial};

#[test]
fn decoding_errors_and_partial_results_match_vanilla() {
    let cases: &[(&str, Option<&str>, Option<&str>)] = &[
        ("{text:\"a\",color:\"nope\"}", Some("Invalid color name: nope"), Some("\"a\"")),
        ("{text:\"a\",bold:\"x\",italic:1b}", Some("Not a number"), Some("{italic:1b,text:\"a\"}")),
        ("{text:\"a\",extra:[{text:\"b\"},{text:1}]}", Some("Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: {text:1}; Second: No matching codec found"), Some("{extra:[\"b\"],text:\"a\"}")),
        ("{text:\"a\",extra:[{text:1}]}", Some("Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: {text:1}; Second: No matching codec found; List must have contents"), Some("\"a\"")),
        ("[{text:\"a\"},{text:1}]", Some("Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: {text:1}; Second: No matching codec found"), Some("\"a\"")),
        ("[{text:1}]", Some("Failed to parse either. First: Failed to parse either. First: Not a string; Second: Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: {text:1}; Second: No matching codec found; List must have contents; Second: Not a map: [{text:1}]"), None),
        ("{text:1}", Some("Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: {text:1}; Second: No matching codec found"), None),
        ("{translate:\"k\",with:[1,{text:1},\"s\"]}", Some("Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: {translate:\"k\",with:[1,{text:1},\"s\"]}; Second: No matching codec found"), None),
        ("{text:\"a\",click_event:{action:\"open_url\"}}", Some("No key url in MapLike[{action:\"open_url\"}]"), Some("\"a\"")),
        ("{text:\"a\",hover_event:{action:\"show_text\",value:{text:\"h\",color:\"bad\"}}}", Some("Invalid color name: bad"), Some("{hover_event:{action:\"show_text\",value:\"h\"},text:\"a\"}")),
        ("{selector:\"@p\",separator:{text:\",\",color:\"bad\"}}", Some("Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: {selector:\"@p\",separator:{color:\"bad\",text:\",\"}}; Second: No matching codec found"), None),
        ("{text:\"a\",extra:\"x\"}", Some("Not a list: \"x\""), Some("\"a\"")),
        ("{text:\"a\",color:\"red\",font:\"Bad Font\"}", Some("Not a valid resource location: Bad Font Non [a-z0-9/._-] character in path of location: minecraft:Bad Font"), Some("{color:\"red\",text:\"a\"}")),
        ("{type:\"text\",text:\"a\",color:\"nope\"}", Some("Invalid color name: nope"), Some("\"a\"")),
        ("{text:\"a\",shadow_color:[1f,2f]}", Some("Failed to parse either. First: Not a number; Second: Input is not a list of 4 elements"), Some("\"a\"")),
        ("{text:\"a\",color:\"nope\",bold:\"x\"}", Some("Not a number; Invalid color name: nope"), Some("\"a\"")),
        ("{text:\"a\",bold:\"x\",color:\"nope\",font:\"A B\"}", Some("Not a valid resource location: A B Non [a-z0-9/._-] character in path of location: minecraft:A B; Not a number; Invalid color name: nope"), Some("\"a\"")),
        ("{text:\"a\",color:\"nope\",extra:[{text:1}]}", Some("Invalid color name: nope; Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: {text:1}; Second: No matching codec found; List must have contents"), Some("\"a\"")),
        ("{text:\"a\",color:\"nope\",extra:[{text:\"b\",color:\"bad\"}]}", Some("Invalid color name: nope; Invalid color name: bad"), Some("{extra:[\"b\"],text:\"a\"}")),
        ("{text:\"a\",extra:[{text:\"b\",color:\"bad\"},{text:\"c\",bold:\"x\"}]}", Some("Not a number; Invalid color name: bad"), Some("{extra:[\"b\",\"c\"],text:\"a\"}")),
        ("[{text:\"a\",color:\"nope\"},{text:\"b\",bold:\"y\"}]", Some("Not a number; Invalid color name: nope"), Some("{extra:[\"b\"],text:\"a\"}")),
        ("[{text:\"a\",color:\"nope\"}]", Some("Invalid color name: nope"), Some("\"a\"")),
        ("{text:\"a\",hover_event:{action:\"show_text\",value:{text:1}}}", Some("Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: {text:1}; Second: No matching codec found"), Some("\"a\"")),
        ("{text:\"a\",hover_event:{action:\"show_text\"}}", Some("No key value in MapLike[{action:\"show_text\"}]"), Some("\"a\"")),
        ("{text:\"a\",hover_event:{action:\"nope\"}}", Some("Unknown element name:nope"), Some("\"a\"")),
        ("{text:\"a\",click_event:{action:\"change_page\",page:0}}", Some("Value must be positive: 0"), Some("\"a\"")),
        ("{text:\"a\",hover_event:{action:\"show_entity\",id:\"pig\",uuid:[I;1,2,3,4],name:{text:\"n\",color:\"bad\"}}}", Some("Invalid color name: bad"), Some("{hover_event:{action:\"show_entity\",id:\"minecraft:pig\",name:\"n\",uuid:[I;1,2,3,4]},text:\"a\"}")),
        ("{text:\"a\",hover_event:{action:\"show_item\",id:\"stone\",count:\"x\"}}", Some("Not a number"), Some("{hover_event:{action:\"show_item\",id:\"minecraft:stone\"},text:\"a\"}")),
        ("{text:\"a\",insertion:1}", Some("Not a string"), Some("\"a\"")),
        ("{text:\"a\",shadow_color:\"x\"}", Some("Failed to parse either. First: Not a number; Second: Not a list: \"x\""), Some("\"a\"")),
        ("{text:\"a\",shadow_color:[1f,2f,3f,4f]}", None, Some("{shadow_color:-50331907,text:\"a\"}")),
        ("{keybind:\"k\",color:\"nope\"}", Some("Invalid color name: nope"), Some("{keybind:\"k\"}")),
    ];
    for &(input, error, partial) in cases {
        let tag = crate::nbt::parse_fully(input).unwrap();
        assert_eq!(decode(&tag).err().as_deref(), error, "{input}");
        let expected = partial.map(|partial| decode(&crate::nbt::parse_fully(partial).unwrap()).unwrap());
        assert_eq!(decode_or_partial(&tag), expected, "{input}");
    }
}

#[test]
fn item_component_text_reports_the_same_errors() {
    let inputs: &[&str] = &["{text:\"a\",color:\"nope\"}", "{text:\"a\",bold:\"x\",italic:1b}", "{text:\"a\",extra:[{text:\"b\"},{text:1}]}", "{text:\"a\",extra:[{text:1}]}", "[{text:\"a\"},{text:1}]", "[{text:1}]", "{text:1}", "{translate:\"k\",with:[1,{text:1},\"s\"]}", "{text:\"a\",click_event:{action:\"open_url\"}}", "{text:\"a\",hover_event:{action:\"show_text\",value:{text:\"h\",color:\"bad\"}}}", "{selector:\"@p\",separator:{text:\",\",color:\"bad\"}}", "{text:\"a\",extra:\"x\"}", "{text:\"a\",color:\"red\",font:\"Bad Font\"}", "{type:\"text\",text:\"a\",color:\"nope\"}", "{text:\"a\",shadow_color:[1f,2f]}", "{text:\"a\",color:\"nope\",bold:\"x\"}", "{text:\"a\",bold:\"x\",color:\"nope\",font:\"A B\"}", "{text:\"a\",color:\"nope\",extra:[{text:1}]}", "{text:\"a\",color:\"nope\",extra:[{text:\"b\",color:\"bad\"}]}", "{text:\"a\",extra:[{text:\"b\",color:\"bad\"},{text:\"c\",bold:\"x\"}]}", "[{text:\"a\",color:\"nope\"},{text:\"b\",bold:\"y\"}]", "[{text:\"a\",color:\"nope\"}]", "{text:\"a\",hover_event:{action:\"show_text\",value:{text:1}}}", "{text:\"a\",hover_event:{action:\"show_text\"}}", "{text:\"a\",hover_event:{action:\"nope\"}}", "{text:\"a\",click_event:{action:\"change_page\",page:0}}", "{text:\"a\",hover_event:{action:\"show_entity\",id:\"pig\",uuid:[I;1,2,3,4],name:{text:\"n\",color:\"bad\"}}}", "{text:\"a\",hover_event:{action:\"show_item\",id:\"stone\",count:\"x\"}}", "{text:\"a\",insertion:1}", "{text:\"a\",shadow_color:\"x\"}", "{text:\"a\",shadow_color:[1f,2f,3f,4f]}", "{keybind:\"k\",color:\"nope\"}"];
    for input in inputs {
        let tag = crate::nbt::parse_fully(input).unwrap();
        assert_eq!(crate::world::item::component::text::decode(&tag).err(), decode(&tag).err(), "{input}");
    }
}
