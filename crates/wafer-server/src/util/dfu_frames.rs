pub const DECODER_PARSE: &str = "com.mojang.serialization.Decoder.parse(Decoder.java:18)";

const FIELD_DECODER_DECODE: &str = "com.mojang.serialization.codecs.FieldDecoder.decode(FieldDecoder.java:29)";
const MAP_CODEC_2_DECODE: &str = "com.mojang.serialization.MapCodec$2.decode(MapCodec.java:78)";
const OPTIONAL_FIELD_CODEC_DECODE: &str = "com.mojang.serialization.codecs.OptionalFieldCodec.decode(OptionalFieldCodec.java:34)";
const RECORD_CODEC_BUILDER_2_DECODE: &str = "com.mojang.serialization.codecs.RecordCodecBuilder$2.decode(RecordCodecBuilder.java:107)";
const DATA_RESULT_SUCCESS_FLAT_MAP: &str = "com.mojang.serialization.DataResult$Success.flatMap(DataResult.java:201)";

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
}
