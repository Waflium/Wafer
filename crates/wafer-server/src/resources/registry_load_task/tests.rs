use super::*;

#[test]
fn a_decode_failure_shares_the_task_frames_with_its_wrapper() {
    let trace = failed_to_parse("ta:x", "file/p", 5, ElementFailure::Decode("Not a string: 5".to_string())).print_stack_trace();
    let lines: Vec<&str> = trace.lines().collect();
    assert_eq!(lines[0], "java.lang.IllegalStateException: Failed to parse ta:x from pack file/p");
    assert_eq!(lines[1], "\tat net.minecraft.resources.RegistryLoadTask$PendingRegistration.loadFromResource(RegistryLoadTask.java:111)");
    assert_eq!(lines[11], "Caused by: java.lang.IllegalStateException: Not a string: 5");
    assert_eq!(lines[12], "\tat com.mojang.serialization.DataResult$Error.getOrThrow(DataResult.java:287)");
    assert_eq!(lines[14], "\tat net.minecraft.resources.RegistryLoadTask$PendingRegistration.loadFromResource(RegistryLoadTask.java:109)");
    assert_eq!(lines[15], "\t... 9 more");
}

#[test]
fn a_single_resource_runs_on_a_supplied_task() {
    let trace = failed_to_parse("ta:x", "file/p", 1, ElementFailure::Decode("m".to_string())).print_stack_trace();
    assert!(trace.contains("ParallelMapTransform.lambda$schedule$0(ParallelMapTransform.java:32)"));
    assert!(trace.contains("CompletableFuture$AsyncSupply.run(CompletableFuture.java:1789)"));
    assert!(trace.contains("\t... 8 more\n"));
}

#[test]
fn a_large_registry_is_split_into_batches() {
    let trace = failed_to_parse("ta:x", "file/p", 100_000, ElementFailure::Decode("m".to_string())).print_stack_trace();
    assert!(trace.contains("ParallelMapTransform$BatchedTaskSplitter.lambda$createTask$0(ParallelMapTransform.java:213)"));
    let trace = failed_to_parse("ta:x", "file/p", 2, ElementFailure::Decode("m".to_string())).print_stack_trace();
    assert!(trace.contains("ParallelMapTransform$SingleTaskSplitter.lambda$scheduleBatch$0(ParallelMapTransform.java:160)"));
}

#[test]
fn a_parse_failure_chains_the_gson_exceptions() {
    let parse_error = crate::util::gson::parse_strict_throwable("{\"a\":").unwrap_err();
    let trace = failed_to_parse("ta:x", "file/p", 2, ElementFailure::Parse(parse_error)).print_stack_trace();
    assert!(trace.contains("Caused by: com.google.gson.JsonSyntaxException: java.io.EOFException: End of input at line 1 column 6 path $.a\n"));
    assert!(trace.contains("\tat net.minecraft.util.StrictJsonParser.parse(StrictJsonParser.java:27)\n\tat net.minecraft.resources.RegistryLoadTask$PendingRegistration.loadFromResource(RegistryLoadTask.java:108)\n\t... 9 more\n"));
    assert!(trace.contains("Caused by: java.io.EOFException: End of input at line 1 column 6 path $.a\n"));
    assert!(trace.ends_with("\t... 12 more\n"));
}

#[test]
fn registry_loading_failures_name_their_stage() {
    let freeze = execution_exception(registry_loading_failed(false)).print_stack_trace();
    assert!(freeze.contains("RegistryDataLoader.lambda$load$2(RegistryDataLoader.java:263)"));
    let validate = execution_exception(registry_loading_failed(true)).print_stack_trace();
    assert!(validate.contains("RegistryDataLoader.lambda$load$2(RegistryDataLoader.java:268)"));
    assert!(validate.ends_with("Caused by: java.lang.IllegalStateException: Failed to load registries due to errors\n\tat net.minecraft.resources.RegistryDataLoader.createReportWithBriefInfo(RegistryDataLoader.java:313)\n\t... 8 more\n"));
}

#[test]
fn an_empty_required_registry_fails_the_validation_stage() {
    let trace = registry_must_be_non_empty("minecraft:wolf_variant").print_stack_trace();
    let lines: Vec<&str> = trace.lines().collect();
    assert_eq!(lines[0], "java.lang.IllegalStateException: Registry must be non-empty: minecraft:wolf_variant");
    assert_eq!(lines[1], "\tat net.minecraft.resources.RegistryValidator.lambda$static$1(RegistryValidator.java:12)");
    assert_eq!(lines[2], "\tat net.minecraft.resources.RegistryLoadTask.validateRegistry(RegistryLoadTask.java:93)");
    assert_eq!(lines[3], "\tat net.minecraft.resources.RegistryDataLoader.lambda$load$4(RegistryDataLoader.java:266)");
    assert!(trace.contains("\tat net.minecraft.resources.RegistryDataLoader.lambda$load$2(RegistryDataLoader.java:266)\n"));
}

#[test]
fn a_function_failure_wraps_the_compile_error_in_a_completion_exception() {
    let trace = function_load_failed("java.lang.IllegalArgumentException: Whilst parsing command on line 1: nope").print_stack_trace();
    assert!(trace.starts_with("java.util.concurrent.CompletionException: java.lang.IllegalArgumentException: Whilst parsing command on line 1: nope\n"));
    assert!(trace.contains("Caused by: java.lang.IllegalArgumentException: Whilst parsing command on line 1: nope\n\tat net.minecraft.commands.functions.CommandFunction.fromLines(CommandFunction.java:81)\n"));
    assert!(trace.ends_with("\t... 8 more\n"));
}

#[test]
fn a_tag_failure_names_where_the_tags_are_loaded() {
    let existing = tag_decode_failed(TagLoadContext::ExistingRegistries, "Not a json array: 5").print_stack_trace();
    assert!(existing.contains("TagLoader.load(TagLoader.java:61)\n\tat net.minecraft.tags.TagLoader.loadPendingTags(TagLoader.java:163)"));
    let dynamic = tag_decode_failed(TagLoadContext::DynamicRegistry, "Not a json array: 5").print_stack_trace();
    assert!(dynamic.contains("TagLoader.loadTagsForRegistry(TagLoader.java:150)"));
    let functions = tag_decode_failed(TagLoadContext::FunctionLibrary, "Not a json array: 5").print_stack_trace();
    assert!(functions.contains("ServerFunctionLibrary.lambda$reload$0(ServerFunctionLibrary.java:71)"));
}

#[test]
fn a_reload_tag_failure_ends_in_the_command_that_reloaded() {
    let invoker = &[
        "java.base/java.util.concurrent.CompletableFuture.uniComposeStage(CompletableFuture.java:1208)",
        "java.base/java.util.concurrent.CompletableFuture.thenCompose(CompletableFuture.java:2368)",
        "net.minecraft.server.MinecraftServer.reloadResources(MinecraftServer.java:1518)",
        "net.minecraft.server.commands.ReloadCommand.reloadPacks(ReloadCommand.java:22)",
        "ta.Command.run(Command.java:1)",
    ];
    let trace = tag_decode_failed(TagLoadContext::ReloadedRegistries(invoker), "Not a json array: 5").print_stack_trace();
    assert!(trace.contains("TagLoader.load(TagLoader.java:61)\n\tat net.minecraft.tags.TagLoader.loadPendingTags(TagLoader.java:163)"));
    assert!(trace.contains("Streams$RangeIntSpliterator.forEachRemaining(Streams.java:104)"));
    assert!(trace.ends_with("MinecraftServer.reloadResources(MinecraftServer.java:1518)\n\tat net.minecraft.server.commands.ReloadCommand.reloadPacks(ReloadCommand.java:22)\n\tat ta.Command.run(Command.java:1)\n"));
}

#[test]
fn unbound_registry_errors_come_from_the_freeze() {
    let values = unbound_values("ResourceKey[minecraft:root / minecraft:worldgen/biome]", &["ta:foo".to_string(), "minecraft:nope".to_string()]).print_stack_trace();
    assert!(values.starts_with("java.lang.IllegalStateException: Unbound values in registry ResourceKey[minecraft:root / minecraft:worldgen/biome]: [ta:foo, minecraft:nope]\n\tat net.minecraft.core.MappedRegistry.freeze(MappedRegistry.java:300)\n"));
    let tags = unbound_tags("ResourceKey[minecraft:root / minecraft:worldgen/carver]", &["ta:nope".to_string()]).print_stack_trace();
    assert!(tags.contains("MappedRegistry.freeze(MappedRegistry.java:315)"));
}
