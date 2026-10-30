pub const DECODER_PARSE: &str = "com.mojang.serialization.Decoder.parse(Decoder.java:18)";

const FIELD_DECODER_DECODE: &str = "com.mojang.serialization.codecs.FieldDecoder.decode(FieldDecoder.java:29)";
const MAP_CODEC_2_DECODE: &str = "com.mojang.serialization.MapCodec$2.decode(MapCodec.java:78)";
const OPTIONAL_FIELD_CODEC_DECODE: &str = "com.mojang.serialization.codecs.OptionalFieldCodec.decode(OptionalFieldCodec.java:34)";
const RECORD_CODEC_BUILDER_2_DECODE: &str = "com.mojang.serialization.codecs.RecordCodecBuilder$2.decode(RecordCodecBuilder.java:107)";
const DATA_RESULT_SUCCESS_FLAT_MAP: &str = "com.mojang.serialization.DataResult$Success.flatMap(DataResult.java:201)";
const MAP_DECODER_4_DECODE: &str = "com.mojang.serialization.MapDecoder$4.decode(MapDecoder.java:94)";
const MAP_DECODER_3_DECODE: &str = "com.mojang.serialization.MapDecoder$3.decode(MapDecoder.java:80)";
const CODEC_3_DECODE: &str = "com.mojang.serialization.Codec$3.decode(Codec.java:102)";
const CODEC_2_DECODE: &str = "com.mojang.serialization.Codec$2.decode(Codec.java:74)";
const DECODER_1_DECODE: &str = "com.mojang.serialization.Decoder$1.decode(Decoder.java:49)";
const DECODER_2_DECODE: &str = "com.mojang.serialization.Decoder$2.decode(Decoder.java:63)";

const CODEC_VIEW: [&str; 4] = [
    "com.mojang.serialization.MapDecoder.lambda$compressedDecode$1(MapDecoder.java:52)",
    DATA_RESULT_SUCCESS_FLAT_MAP,
    "com.mojang.serialization.MapDecoder.compressedDecode(MapDecoder.java:52)",
    "com.mojang.serialization.MapCodec$MapCodecCodec.decode(MapCodec.java:159)",
];

const LIST: [&str; 5] = [
    "com.mojang.serialization.codecs.ListCodec$DecoderState.accept(ListCodec.java:74)",
    "com.mojang.serialization.JsonOps.lambda$getList$21(JsonOps.java:308)",
    "com.mojang.serialization.codecs.ListCodec.lambda$decode$2(ListCodec.java:45)",
    DATA_RESULT_SUCCESS_FLAT_MAP,
    "com.mojang.serialization.codecs.ListCodec.decode(ListCodec.java:43)",
];

const MAP_VALUES: [&str; 12] = [
    "com.mojang.serialization.codecs.BaseMapCodec.lambda$decode$3(BaseMapCodec.java:34)",
    "java.base/java.util.stream.ReduceOps$1ReducingSink.accept(ReduceOps.java:80)",
    "java.base/java.util.stream.ReferencePipeline$3$1.accept(ReferencePipeline.java:214)",
    "java.base/java.util.Iterator.forEachRemaining(Iterator.java:133)",
    "java.base/java.util.Spliterators$IteratorSpliterator.forEachRemaining(Spliterators.java:1939)",
    "java.base/java.util.stream.AbstractPipeline.copyInto(AbstractPipeline.java:570)",
    "java.base/java.util.stream.AbstractPipeline.wrapAndCopyInto(AbstractPipeline.java:560)",
    "java.base/java.util.stream.ReduceOps$ReduceOp.evaluateSequential(ReduceOps.java:921)",
    "java.base/java.util.stream.AbstractPipeline.evaluate(AbstractPipeline.java:265)",
    "java.base/java.util.stream.ReferencePipeline.reduce(ReferencePipeline.java:703)",
    "com.mojang.serialization.codecs.BaseMapCodec.decode(BaseMapCodec.java:30)",
    "com.mojang.serialization.codecs.SimpleMapCodec.decode(SimpleMapCodec.java:48)",
];

const KEY_DISPATCH: [&str; 5] = [
    "com.mojang.serialization.codecs.KeyDispatchCodec.lambda$decode$2(KeyDispatchCodec.java:46)",
    DATA_RESULT_SUCCESS_FLAT_MAP,
    "com.mojang.serialization.codecs.KeyDispatchCodec.lambda$decode$3(KeyDispatchCodec.java:38)",
    DATA_RESULT_SUCCESS_FLAT_MAP,
    "com.mojang.serialization.codecs.KeyDispatchCodec.decode(KeyDispatchCodec.java:37)",
];

pub fn required_field() -> Vec<&'static str> {
    vec![DECODER_PARSE, FIELD_DECODER_DECODE, MAP_CODEC_2_DECODE]
}

pub fn optional_field() -> Vec<&'static str> {
    vec![DECODER_PARSE, OPTIONAL_FIELD_CODEC_DECODE]
}

pub fn optional_field_with_default() -> Vec<&'static str> {
    vec![DECODER_PARSE, OPTIONAL_FIELD_CODEC_DECODE, MAP_DECODER_4_DECODE, MAP_CODEC_2_DECODE]
}

pub fn codec_xmap() -> Vec<&'static str> {
    vec![DECODER_2_DECODE, CODEC_2_DECODE]
}

pub fn codec_flat_xmap() -> Vec<&'static str> {
    vec![DECODER_1_DECODE, CODEC_2_DECODE]
}

pub fn stable() -> Vec<&'static str> {
    vec!["com.mojang.serialization.Codec$1.decode(Codec.java:48)"]
}

pub fn promote_partial() -> Vec<&'static str> {
    vec!["com.mojang.serialization.Decoder$3.decode(Decoder.java:77)", CODEC_2_DECODE]
}

pub fn map_codec_xmap() -> Vec<&'static str> {
    vec![MAP_DECODER_4_DECODE, MAP_CODEC_2_DECODE]
}

pub fn map_codec_flat_xmap() -> Vec<&'static str> {
    vec![MAP_DECODER_3_DECODE, CODEC_3_DECODE]
}

pub fn either_first() -> Vec<&'static str> {
    vec!["com.mojang.serialization.codecs.EitherCodec.decode(EitherCodec.java:14)"]
}

pub fn either_second() -> Vec<&'static str> {
    vec!["com.mojang.serialization.codecs.EitherCodec.decode(EitherCodec.java:18)"]
}

pub fn xor_first() -> Vec<&'static str> {
    vec!["com.mojang.serialization.codecs.XorCodec.decode(XorCodec.java:16)"]
}

pub fn xor_second() -> Vec<&'static str> {
    vec!["com.mojang.serialization.codecs.XorCodec.decode(XorCodec.java:17)"]
}

pub fn dispatch_optional_value() -> Vec<&'static str> {
    vec!["net.minecraft.util.ExtraCodecs$6.lambda$decode$1(ExtraCodecs.java:767)", DATA_RESULT_SUCCESS_FLAT_MAP, "net.minecraft.util.ExtraCodecs$6.decode(ExtraCodecs.java:765)"]
}

pub fn holder_set_values() -> Vec<&'static str> {
    vec![
        "com.mojang.serialization.codecs.EitherCodec.decode(EitherCodec.java:18)",
        "net.minecraft.core.registries.codec.HolderSetCodec.decode(HolderSetCodec.java:51)",
    ]
}

pub fn flat_restricted() -> Vec<&'static str> {
    vec!["net.minecraft.network.chat.ComponentSerialization$1.decode(ComponentSerialization.java:53)"]
}

pub fn recursive_map() -> Vec<&'static str> {
    vec!["com.mojang.serialization.MapCodec$RecursiveMapCodec.decode(MapCodec.java:113)"]
}

pub fn recursive() -> Vec<&'static str> {
    vec!["com.mojang.serialization.Codec$RecursiveCodec.decode(Codec.java:219)"]
}

pub fn registry_file_inline() -> Vec<&'static str> {
    vec!["net.minecraft.core.registries.codec.RegistryFileCodec.decode(RegistryFileCodec.java:64)"]
}

pub fn pair_map_first() -> Vec<&'static str> {
    vec!["com.mojang.serialization.codecs.PairMapCodec.decode(PairMapCodec.java:26)"]
}

pub fn pair_map_second() -> Vec<&'static str> {
    vec!["com.mojang.serialization.codecs.PairMapCodec.lambda$decode$1(PairMapCodec.java:27)", DATA_RESULT_SUCCESS_FLAT_MAP, "com.mojang.serialization.codecs.PairMapCodec.decode(PairMapCodec.java:26)"]
}

#[derive(Clone, Copy)]
pub enum FieldShape {
    Required,
    Optional,
    OptionalDefault,
    Nested,
}

pub fn record_field(shape: FieldShape, size: usize, index: usize) -> Vec<&'static str> {
    let mut frames = match shape {
        FieldShape::Nested => Vec::new(),
        FieldShape::Required => required_field(),
        FieldShape::Optional => optional_field(),
        FieldShape::OptionalDefault => optional_field_with_default(),
    };
    frames.extend(record_group(size, index + 1));
    frames.push(RECORD_CODEC_BUILDER_2_DECODE);
    frames
}

pub fn record_map_codec() -> &'static str {
    RECORD_CODEC_BUILDER_2_DECODE
}

pub fn codec_view() -> Vec<&'static str> {
    CODEC_VIEW.to_vec()
}

pub fn list() -> Vec<&'static str> {
    LIST.to_vec()
}

pub fn mapped() -> Vec<&'static str> {
    vec!["com.mojang.serialization.Decoder$2.decode(Decoder.java:63)", "com.mojang.serialization.Codec$2.decode(Codec.java:74)"]
}

pub fn unbounded_map_values() -> Vec<&'static str> {
    let mut frames = vec![DECODER_PARSE];
    frames.extend(&MAP_VALUES[..11]);
    frames.push("com.mojang.serialization.codecs.UnboundedMapCodec.lambda$decode$0(UnboundedMapCodec.java:22)");
    frames.push(DATA_RESULT_SUCCESS_FLAT_MAP);
    frames.push("com.mojang.serialization.codecs.UnboundedMapCodec.decode(UnboundedMapCodec.java:22)");
    frames
}

pub fn dispatched_map_values() -> Vec<&'static str> {
    let mut frames = vec![
        DECODER_PARSE,
        "com.mojang.serialization.codecs.DispatchedMapCodec.lambda$parseEntry$6(DispatchedMapCodec.java:59)",
        DATA_RESULT_SUCCESS_FLAT_MAP,
        "com.mojang.serialization.codecs.DispatchedMapCodec.parseEntry(DispatchedMapCodec.java:59)",
        "com.mojang.serialization.codecs.DispatchedMapCodec.lambda$decode$0(DispatchedMapCodec.java:46)",
    ];
    frames.extend(&MAP_VALUES[1..10]);
    frames.push("com.mojang.serialization.codecs.DispatchedMapCodec.lambda$decode$5(DispatchedMapCodec.java:44)");
    frames.push(DATA_RESULT_SUCCESS_FLAT_MAP);
    frames.push("com.mojang.serialization.codecs.DispatchedMapCodec.decode(DispatchedMapCodec.java:40)");
    frames
}

pub fn encoder_cache() -> Vec<&'static str> {
    vec!["net.minecraft.util.EncoderCache$2.decode(EncoderCache.java:34)"]
}

pub fn map_values() -> Vec<&'static str> {
    let mut frames = vec![DECODER_PARSE];
    frames.extend(MAP_VALUES);
    frames.extend(CODEC_VIEW);
    frames
}

pub fn key_dispatch() -> Vec<&'static str> {
    KEY_DISPATCH.to_vec()
}

const INT_READ: [&str; 2] = [
    "com.mojang.serialization.DataResult$Success.map(DataResult.java:175)",
    "com.mojang.serialization.Codec$12.read(Codec.java:563)",
];

const LONG_READ: [&str; 2] = [
    "com.mojang.serialization.DataResult$Success.map(DataResult.java:175)",
    "com.mojang.serialization.Codec$13.read(Codec.java:582)",
];

pub fn byte_leaf() -> Vec<&'static str> {
    primitive_leaf(["com.mojang.serialization.DataResult$Success.map(DataResult.java:175)", "com.mojang.serialization.Codec$10.read(Codec.java:525)"], false)
}

pub fn short_leaf() -> Vec<&'static str> {
    primitive_leaf(["com.mojang.serialization.DataResult$Success.map(DataResult.java:175)", "com.mojang.serialization.Codec$11.read(Codec.java:544)"], false)
}

pub fn int_leaf(ranged: bool) -> Vec<&'static str> {
    primitive_leaf(INT_READ, ranged)
}

pub fn long_leaf(ranged: bool) -> Vec<&'static str> {
    primitive_leaf(LONG_READ, ranged)
}

fn primitive_leaf(read: [&'static str; 2], ranged: bool) -> Vec<&'static str> {
    let mut frames = read.to_vec();
    frames.push("com.mojang.serialization.codecs.PrimitiveCodec.decode(PrimitiveCodec.java:17)");
    if ranged {
        frames.push("com.mojang.serialization.Decoder$1.decode(Decoder.java:49)");
        frames.push("com.mojang.serialization.Codec$2.decode(Codec.java:74)");
    }
    frames
}

const MAX_GROUP: usize = 16;

pub fn record_group(size: usize, index: usize) -> Vec<&'static str> {
    let mut frames = Vec::new();
    if (1..=MAX_GROUP).contains(&size) && (1..=size).contains(&index) {
        group(size, index, &mut frames);
    }
    frames
}

fn outer_arity(size: usize) -> usize {
    (size + 1) / 2
}

fn group(size: usize, index: usize, frames: &mut Vec<&'static str>) {
    if size <= 4 {
        frames.push(applicative(size, index));
        return;
    }
    let outer = outer_arity(size);
    let inner = size - outer;
    if index > inner {
        group(outer, index - inner, frames);
    } else {
        group(inner, index, frames);
        function(outer, frames);
    }
}

fn function(size: usize, frames: &mut Vec<&'static str>) {
    if size <= 4 {
        frames.push(applicative(size, 0));
        return;
    }
    frames.push(MAP_DECODER_4_DECODE);
    let outer = outer_arity(size);
    function(size - outer, frames);
    function(outer, frames);
}

fn applicative(size: usize, index: usize) -> &'static str {
    match (size, index) {
        (1, _) => "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$1.decode(RecordCodecBuilder.java:183)",
        (2, 0) => "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$3.decode(RecordCodecBuilder.java:247)",
        (2, 1) => "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$3.decode(RecordCodecBuilder.java:248)",
        (2, _) => "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$3.decode(RecordCodecBuilder.java:249)",
        (3, 0) => "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$5.decode(RecordCodecBuilder.java:322)",
        (3, 1) => "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$5.decode(RecordCodecBuilder.java:323)",
        (3, 2) => "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$5.decode(RecordCodecBuilder.java:324)",
        (3, _) => "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$5.decode(RecordCodecBuilder.java:325)",
        (_, 0) => "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$7.decode(RecordCodecBuilder.java:405)",
        (_, 1) => "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$7.decode(RecordCodecBuilder.java:406)",
        (_, 2) => "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$7.decode(RecordCodecBuilder.java:407)",
        (_, 3) => "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$7.decode(RecordCodecBuilder.java:408)",
        (_, _) => "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$7.decode(RecordCodecBuilder.java:409)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(frames: Vec<&'static str>) -> Vec<String> {
        frames.iter().map(|frame| frame.rsplit_once(':').unwrap().1.trim_end_matches(')').to_string()).collect()
    }

    #[test]
    fn small_records_use_one_applicative_class_with_a_line_per_field() {
        assert_eq!(lines(record_group(1, 1)), ["183"]);
        assert_eq!(lines(record_group(2, 2)), ["249"]);
        assert_eq!(lines(record_group(3, 1)), ["323"]);
        assert_eq!(lines(record_group(4, 4)), ["409"]);
        assert_eq!(lines(record_group(4, 2)), ["407"]);
    }

    #[test]
    fn larger_records_nest_the_applicatives_like_ap5_to_ap8() {
        assert_eq!(lines(record_group(5, 5)), ["325"]);
        assert_eq!(lines(record_group(5, 1)), ["248", "322"]);
        assert_eq!(lines(record_group(6, 6)), ["325"]);
        assert_eq!(lines(record_group(8, 3)), ["408", "405"]);
        assert_eq!(lines(record_group(8, 8)), ["409"]);
    }

    #[test]
    fn mapping_the_function_of_a_nested_applicative_adds_a_map_decoder_frame() {
        assert_eq!(lines(record_group(11, 4)), ["324", "94", "322", "322"]);
        assert_eq!(lines(record_group(8, 4)), ["409", "405"]);
    }
}
