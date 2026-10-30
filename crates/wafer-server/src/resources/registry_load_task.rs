use crate::util::java_throwable::JavaThrowable;

const ILLEGAL_STATE_EXCEPTION: &str = "java.lang.IllegalStateException";

const FORK_JOIN_TAIL: [&str; 4] = [
    "java.base/java.util.concurrent.ForkJoinTask.doExec(ForkJoinTask.java:511)",
    "java.base/java.util.concurrent.ForkJoinPool$WorkQueue.topLevelExec(ForkJoinPool.java:1450)",
    "java.base/java.util.concurrent.ForkJoinPool.runWorker(ForkJoinPool.java:2019)",
    "java.base/java.util.concurrent.ForkJoinWorkerThread.run(ForkJoinWorkerThread.java:187)",
];

const LOAD_LAMBDA: &str = "net.minecraft.resources.ResourceManagerRegistryLoadTask.lambda$load$2(ResourceManagerRegistryLoadTask.java:46)";
const APPLY_OPERATION: &str = "net.minecraft.util.thread.ParallelMapTransform$Container.applyOperation(ParallelMapTransform.java:89)";
const SINGLE_ELEMENT_LAMBDA: &str = "net.minecraft.util.thread.ParallelMapTransform.lambda$schedule$0(ParallelMapTransform.java:32)";
const SINGLE_TASK_LAMBDA: &str = "net.minecraft.util.thread.ParallelMapTransform$SingleTaskSplitter.lambda$scheduleBatch$0(ParallelMapTransform.java:160)";
const BATCHED_TASK_LAMBDA: &str = "net.minecraft.util.thread.ParallelMapTransform$BatchedTaskSplitter.lambda$createTask$0(ParallelMapTransform.java:213)";
const ASYNC_SUPPLY: [&str; 2] = [
    "java.base/java.util.concurrent.CompletableFuture$AsyncSupply.run(CompletableFuture.java:1789)",
    "java.base/java.util.concurrent.CompletableFuture$AsyncSupply.exec(CompletableFuture.java:1781)",
];
const ASYNC_RUN: [&str; 2] = [
    "java.base/java.util.concurrent.CompletableFuture$AsyncRun.run(CompletableFuture.java:1825)",
    "java.base/java.util.concurrent.CompletableFuture$AsyncRun.exec(CompletableFuture.java:1817)",
];
const UNI_APPLY: [&str; 2] = [
    "java.base/java.util.concurrent.CompletableFuture$UniApply.tryFire(CompletableFuture.java:667)",
    "java.base/java.util.concurrent.CompletableFuture$Completion.exec(CompletableFuture.java:504)",
];

const LOAD_FROM_RESOURCE_108: &str = "net.minecraft.resources.RegistryLoadTask$PendingRegistration.loadFromResource(RegistryLoadTask.java:108)";
const LOAD_FROM_RESOURCE_109: &str = "net.minecraft.resources.RegistryLoadTask$PendingRegistration.loadFromResource(RegistryLoadTask.java:109)";
const LOAD_FROM_RESOURCE_111: &str = "net.minecraft.resources.RegistryLoadTask$PendingRegistration.loadFromResource(RegistryLoadTask.java:111)";

const DATA_RESULT_GET_OR_THROW: [&str; 2] = [
    "com.mojang.serialization.DataResult$Error.getOrThrow(DataResult.java:287)",
    "com.mojang.serialization.DataResult.getOrThrow(DataResult.java:81)",
];

const FREEZE_REGISTRY: &str = "net.minecraft.resources.RegistryLoadTask.freezeRegistry(RegistryLoadTask.java:83)";
const FREEZE_LAMBDA: &str = "net.minecraft.resources.RegistryDataLoader.lambda$load$3(RegistryDataLoader.java:261)";
const FREEZE_PIPELINE: [&str; 9] = [
    "java.base/java.util.stream.ReferencePipeline$2$1.accept(ReferencePipeline.java:196)",
    "java.base/java.util.AbstractList$RandomAccessSpliterator.forEachRemaining(AbstractList.java:722)",
    "java.base/java.util.stream.AbstractPipeline.copyInto(AbstractPipeline.java:570)",
    "java.base/java.util.stream.AbstractPipeline.wrapAndCopyInto(AbstractPipeline.java:560)",
    "java.base/java.util.stream.AbstractPipeline.evaluate(AbstractPipeline.java:635)",
    "java.base/java.util.stream.AbstractPipeline.evaluateToArrayNode(AbstractPipeline.java:291)",
    "java.base/java.util.stream.ReferencePipeline.toArray(ReferencePipeline.java:652)",
    "java.base/java.util.stream.ReferencePipeline.toArray(ReferencePipeline.java:658)",
    "java.base/java.util.stream.ReferencePipeline.toList(ReferencePipeline.java:663)",
];

const VALIDATE_PIPELINE: [&str; 9] = [
    "java.base/java.util.stream.ReferencePipeline$7$1FlatMap.accept(ReferencePipeline.java:288)",
    "java.base/java.util.AbstractList$RandomAccessSpliterator.forEachRemaining(AbstractList.java:722)",
    "java.base/java.util.stream.AbstractPipeline.copyInto(AbstractPipeline.java:570)",
    "java.base/java.util.stream.AbstractPipeline.wrapAndCopyInto(AbstractPipeline.java:560)",
    "java.base/java.util.stream.AbstractPipeline.evaluate(AbstractPipeline.java:635)",
    "java.base/java.util.stream.AbstractPipeline.evaluateToArrayNode(AbstractPipeline.java:291)",
    "java.base/java.util.stream.ReferencePipeline.toArray(ReferencePipeline.java:652)",
    "java.base/java.util.stream.ReferencePipeline.toArray(ReferencePipeline.java:658)",
    "java.base/java.util.stream.ReferencePipeline.toList(ReferencePipeline.java:663)",
];

const TIMELINE_VALIDATE: [&str; 4] = [
    "net.minecraft.world.timeline.Timeline.lambda$validateRegistry$0(Timeline.java:74)",
    "it.unimi.dsi.fastutil.objects.ObjectArrayList$Spliterator.forEachRemaining(ObjectArrayList.java:975)",
    "java.base/java.util.stream.ReferencePipeline$Head.forEach(ReferencePipeline.java:803)",
    "net.minecraft.world.timeline.Timeline.validateRegistry(Timeline.java:70)",
];

const VALIDATE_REGISTRY: &str = "net.minecraft.resources.RegistryLoadTask.validateRegistry(RegistryLoadTask.java:93)";
const VALIDATE_LAMBDA: &str = "net.minecraft.resources.RegistryDataLoader.lambda$load$4(RegistryDataLoader.java:266)";
const LOAD_COMPLETION_VALIDATE: &str = "net.minecraft.resources.RegistryDataLoader.lambda$load$2(RegistryDataLoader.java:266)";
const LOAD_COMPLETION_FREEZE: &str = "net.minecraft.resources.RegistryDataLoader.lambda$load$2(RegistryDataLoader.java:261)";

const LOG_ERRORS: &str = "net.minecraft.resources.RegistryDataLoader.logErrors(RegistryDataLoader.java:294)";
const CREATE_REPORT_WITH_BRIEF_INFO_313: &str = "net.minecraft.resources.RegistryDataLoader.createReportWithBriefInfo(RegistryDataLoader.java:313)";
const CREATE_REPORT_WITH_BRIEF_INFO_329: &str = "net.minecraft.resources.RegistryDataLoader.createReportWithBriefInfo(RegistryDataLoader.java:329)";
const LOAD_COMPLETION_FREEZE_ERRORS: &str = "net.minecraft.resources.RegistryDataLoader.lambda$load$2(RegistryDataLoader.java:263)";
const LOAD_COMPLETION_VALIDATE_ERRORS: &str = "net.minecraft.resources.RegistryDataLoader.lambda$load$2(RegistryDataLoader.java:268)";

const EXECUTION_EXCEPTION_FRAMES: [&str; 4] = [
    "java.base/java.util.concurrent.CompletableFuture.wrapInExecutionException(CompletableFuture.java:345)",
    "java.base/java.util.concurrent.CompletableFuture.reportGet(CompletableFuture.java:440)",
    "java.base/java.util.concurrent.CompletableFuture.get(CompletableFuture.java:2094)",
    "net.minecraft.server.Main.main(Main.java:198)",
];

const LOOT_VALIDATION: [&str; 3] = [
    "net.minecraft.server.ReloadableServerRegistries.validateLootRegistries(ReloadableServerRegistries.java:71)",
    "net.minecraft.server.ReloadableServerRegistries.createAndValidateFullContext(ReloadableServerRegistries.java:52)",
    "net.minecraft.server.ReloadableServerRegistries.lambda$reload$0(ReloadableServerRegistries.java:44)",
];

const COMPLETION_EXCEPTION_FRAMES: [&str; 5] = [
    "java.base/java.util.concurrent.CompletableFuture.wrapInCompletionException(CompletableFuture.java:323)",
    "java.base/java.util.concurrent.CompletableFuture.encodeThrowable(CompletableFuture.java:359)",
    "java.base/java.util.concurrent.CompletableFuture.completeThrowable(CompletableFuture.java:364)",
    "java.base/java.util.concurrent.CompletableFuture$UniApply.tryFire(CompletableFuture.java:670)",
    "java.base/java.util.concurrent.CompletableFuture$Completion.exec(CompletableFuture.java:504)",
];

fn parallel_map_transform_frames(resource_count: usize) -> Vec<&'static str> {
    let mut frames = vec![LOAD_LAMBDA];
    if resource_count == 1 {
        frames.push(SINGLE_ELEMENT_LAMBDA);
        frames.extend(ASYNC_SUPPLY);
    } else {
        frames.push(APPLY_OPERATION);
        if resource_count <= crate::util::max_allowed_executor_threads() * 16 {
            frames.push(SINGLE_TASK_LAMBDA);
        } else {
            frames.push(BATCHED_TASK_LAMBDA);
        }
        frames.extend(ASYNC_RUN);
    }
    frames.extend(FORK_JOIN_TAIL);
    frames
}

fn with_frames(first: &[&'static str], rest: &[&'static str]) -> Vec<&'static str> {
    let mut frames = first.to_vec();
    frames.extend_from_slice(rest);
    frames
}

fn append_frames(throwable: &mut JavaThrowable, frames: &[&'static str]) {
    throwable.stack_trace.extend_from_slice(frames);
    if let Some(cause) = throwable.cause.as_deref_mut() {
        append_frames(cause, frames);
    }
}

pub enum ElementFailure {
    Parse(JavaThrowable),
    Decode(String),
    Throw(JavaThrowable),
}

pub fn failed_to_parse(element_id: &str, pack_id: &str, resource_count: usize, failure: ElementFailure) -> JavaThrowable {
    let task_frames = parallel_map_transform_frames(resource_count);
    let cause = match failure {
        ElementFailure::Parse(mut parse_error) => {
            append_frames(&mut parse_error, &with_frames(&[LOAD_FROM_RESOURCE_108], &task_frames));
            parse_error
        }
        ElementFailure::Throw(mut thrown) => {
            append_frames(&mut thrown, &with_frames(&[LOAD_FROM_RESOURCE_109], &task_frames));
            thrown
        }
        ElementFailure::Decode(message) => {
            JavaThrowable::new(format!("{ILLEGAL_STATE_EXCEPTION}: {message}"), with_frames(&[DATA_RESULT_GET_OR_THROW[0], DATA_RESULT_GET_OR_THROW[1], LOAD_FROM_RESOURCE_109], &task_frames))
        }
    };
    JavaThrowable::new(format!("{ILLEGAL_STATE_EXCEPTION}: Failed to parse {element_id} from pack {pack_id}"), with_frames(&[LOAD_FROM_RESOURCE_111], &task_frames)).with_cause(cause)
}

pub fn unbound_values(registry_key: &str, keys: &[String]) -> JavaThrowable {
    freeze_failure("net.minecraft.core.MappedRegistry.freeze(MappedRegistry.java:300)", format!("Unbound values in registry {registry_key}: [{}]", keys.join(", ")))
}

pub fn unbound_tags(registry_key: &str, keys: &[String]) -> JavaThrowable {
    freeze_failure("net.minecraft.core.MappedRegistry.freeze(MappedRegistry.java:315)", format!("Unbound tags in registry {registry_key}: [{}]", keys.join(", ")))
}

fn freeze_failure(freeze: &'static str, message: String) -> JavaThrowable {
    let mut frames = vec![freeze, FREEZE_REGISTRY, FREEZE_LAMBDA];
    frames.extend(FREEZE_PIPELINE);
    frames.push(LOAD_COMPLETION_FREEZE);
    frames.extend(UNI_APPLY);
    frames.extend(FORK_JOIN_TAIL);
    JavaThrowable::new(format!("{ILLEGAL_STATE_EXCEPTION}: {message}"), frames)
}

pub fn timeline_marker_defined_multiple_times(message: String) -> JavaThrowable {
    let mut frames = TIMELINE_VALIDATE.to_vec();
    frames.push(VALIDATE_REGISTRY);
    frames.push(VALIDATE_LAMBDA);
    frames.extend(VALIDATE_PIPELINE);
    frames.push(LOAD_COMPLETION_VALIDATE);
    frames.extend(UNI_APPLY);
    frames.extend(FORK_JOIN_TAIL);
    JavaThrowable::new(format!("{ILLEGAL_STATE_EXCEPTION}: {message}"), frames)
}

const REGISTRY_VALIDATOR_NON_EMPTY: &str = "net.minecraft.resources.RegistryValidator.lambda$static$1(RegistryValidator.java:12)";

pub fn registry_must_be_non_empty(registry: &str) -> JavaThrowable {
    let mut frames = vec![REGISTRY_VALIDATOR_NON_EMPTY, VALIDATE_REGISTRY, VALIDATE_LAMBDA];
    frames.extend(VALIDATE_PIPELINE);
    frames.push(LOAD_COMPLETION_VALIDATE);
    frames.extend(UNI_APPLY);
    frames.extend(FORK_JOIN_TAIL);
    JavaThrowable::new(format!("{ILLEGAL_STATE_EXCEPTION}: Registry must be non-empty: {registry}"), frames)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagLoadContext<'a> {
    ExistingRegistries,
    ReloadedRegistries(&'a [&'static str]),
    DynamicRegistry,
    FunctionLibrary,
}

const TAG_LOAD_PENDING_TAGS: &str = "net.minecraft.tags.TagLoader.loadPendingTags(TagLoader.java:163)";
const TAG_LOAD_EXISTING_LAMBDA: &str = "net.minecraft.tags.TagLoader.lambda$loadTagsForExistingRegistries$0(TagLoader.java:139)";
const TAG_LOAD_EXISTING_PIPELINE: [&str; 11] = [
    "java.base/java.util.stream.ReferencePipeline$3$1.accept(ReferencePipeline.java:214)",
    "java.base/java.util.stream.ReferencePipeline$3$1.accept(ReferencePipeline.java:214)",
    "com.google.common.collect.CollectSpliterators$1.lambda$forEachRemaining$0(CollectSpliterators.java:125)",
    "java.base/java.util.HashMap$EntrySpliterator.forEachRemaining(HashMap.java:1859)",
    "com.google.common.collect.CollectSpliterators$1.forEachRemaining(CollectSpliterators.java:125)",
    "java.base/java.util.stream.AbstractPipeline.copyInto(AbstractPipeline.java:570)",
    "java.base/java.util.stream.AbstractPipeline.wrapAndCopyInto(AbstractPipeline.java:560)",
    "java.base/java.util.stream.ReduceOps$ReduceOp.evaluateSequential(ReduceOps.java:921)",
    "java.base/java.util.stream.AbstractPipeline.evaluate(AbstractPipeline.java:265)",
    "java.base/java.util.stream.ReferencePipeline.collect(ReferencePipeline.java:723)",
    "net.minecraft.tags.TagLoader.loadTagsForExistingRegistries(TagLoader.java:141)",
];
const TAG_LOAD_RELOAD_PIPELINE: [&str; 11] = [
    "java.base/java.util.stream.ReferencePipeline$3$1.accept(ReferencePipeline.java:214)",
    "java.base/java.util.stream.ReferencePipeline$3$1.accept(ReferencePipeline.java:214)",
    "com.google.common.collect.CollectSpliterators$1WithCharacteristics.lambda$forEachRemaining$0(CollectSpliterators.java:70)",
    "java.base/java.util.stream.Streams$RangeIntSpliterator.forEachRemaining(Streams.java:104)",
    "com.google.common.collect.CollectSpliterators$1WithCharacteristics.forEachRemaining(CollectSpliterators.java:70)",
    "java.base/java.util.stream.AbstractPipeline.copyInto(AbstractPipeline.java:570)",
    "java.base/java.util.stream.AbstractPipeline.wrapAndCopyInto(AbstractPipeline.java:560)",
    "java.base/java.util.stream.ReduceOps$ReduceOp.evaluateSequential(ReduceOps.java:921)",
    "java.base/java.util.stream.AbstractPipeline.evaluate(AbstractPipeline.java:265)",
    "java.base/java.util.stream.ReferencePipeline.collect(ReferencePipeline.java:723)",
    "net.minecraft.tags.TagLoader.loadTagsForExistingRegistries(TagLoader.java:141)",
];
const TAG_LOAD_RELOAD_RESOURCES: [&str; 1] = ["net.minecraft.server.MinecraftServer.lambda$reloadResources$1(MinecraftServer.java:1522)"];
const TAG_LOAD_WORLD_LOADER: [&str; 4] = [
    "net.minecraft.server.WorldLoader.lambda$load$0(WorldLoader.java:33)",
    "java.base/java.util.concurrent.CompletableFuture$UniCompose.tryFire(CompletableFuture.java:1171)",
    "java.base/java.util.concurrent.CompletableFuture$Completion.exec(CompletableFuture.java:504)",
    FORK_JOIN_TAIL[0],
];
const TAG_LOAD_REGISTRY: [&str; 4] = [
    "net.minecraft.tags.TagLoader.loadTagsForRegistry(TagLoader.java:150)",
    "net.minecraft.resources.ResourceManagerRegistryLoadTask.lambda$load$3(ResourceManagerRegistryLoadTask.java:62)",
    "java.base/java.util.concurrent.CompletableFuture$UniAccept.tryFire(CompletableFuture.java:739)",
    "java.base/java.util.concurrent.CompletableFuture$Completion.exec(CompletableFuture.java:504)",
];
const TAG_LOAD_FUNCTIONS: [&str; 6] = [
    "net.minecraft.server.ServerFunctionLibrary.lambda$reload$0(ServerFunctionLibrary.java:71)",
    ASYNC_SUPPLY[0],
    "net.minecraft.server.packs.resources.SimpleReloadInstance.lambda$prepareTasks$1(SimpleReloadInstance.java:51)",
    "java.base/java.util.concurrent.ForkJoinTask$RunnableExecuteAction.compute(ForkJoinTask.java:1753)",
    "java.base/java.util.concurrent.ForkJoinTask$RunnableExecuteAction.compute(ForkJoinTask.java:1745)",
    "java.base/java.util.concurrent.ForkJoinTask$InterruptibleTask.exec(ForkJoinTask.java:1662)",
];

fn tag_load_frames(context: TagLoadContext<'_>, line: &'static str) -> Vec<&'static str> {
    let mut frames = vec![line];
    match context {
        TagLoadContext::ExistingRegistries => {
            frames.push(TAG_LOAD_PENDING_TAGS);
            frames.push(TAG_LOAD_EXISTING_LAMBDA);
            frames.extend(TAG_LOAD_EXISTING_PIPELINE);
            frames.extend(TAG_LOAD_WORLD_LOADER);
            frames.extend(&FORK_JOIN_TAIL[1..]);
        }
        TagLoadContext::ReloadedRegistries(invoker) => {
            frames.push(TAG_LOAD_PENDING_TAGS);
            frames.push(TAG_LOAD_EXISTING_LAMBDA);
            frames.extend(TAG_LOAD_RELOAD_PIPELINE);
            frames.extend(TAG_LOAD_RELOAD_RESOURCES);
            frames.extend_from_slice(invoker);
        }
        TagLoadContext::DynamicRegistry => {
            frames.extend(TAG_LOAD_REGISTRY);
            frames.extend(FORK_JOIN_TAIL);
        }
        TagLoadContext::FunctionLibrary => {
            frames.extend(TAG_LOAD_FUNCTIONS);
            frames.extend(FORK_JOIN_TAIL);
        }
    }
    frames
}

const FUNCTION_COMPLETION_FRAMES: [&str; 8] = [
    "java.base/java.util.concurrent.CompletableFuture.wrapInCompletionException(CompletableFuture.java:323)",
    "java.base/java.util.concurrent.CompletableFuture.encodeThrowable(CompletableFuture.java:359)",
    "java.base/java.util.concurrent.CompletableFuture.completeThrowable(CompletableFuture.java:364)",
    "java.base/java.util.concurrent.CompletableFuture$AsyncSupply.run(CompletableFuture.java:1791)",
    "net.minecraft.server.packs.resources.SimpleReloadInstance.lambda$prepareTasks$1(SimpleReloadInstance.java:51)",
    "java.base/java.util.concurrent.ForkJoinTask$RunnableExecuteAction.compute(ForkJoinTask.java:1753)",
    "java.base/java.util.concurrent.ForkJoinTask$RunnableExecuteAction.compute(ForkJoinTask.java:1745)",
    "java.base/java.util.concurrent.ForkJoinTask$InterruptibleTask.exec(ForkJoinTask.java:1662)",
];

const FUNCTION_COMPILE_FRAMES: [&str; 4] = [
    "net.minecraft.commands.functions.CommandFunction.fromLines(CommandFunction.java:81)",
    "net.minecraft.server.ServerFunctionLibrary.lambda$reload$3(ServerFunctionLibrary.java:85)",
    ASYNC_SUPPLY[0],
    "net.minecraft.server.packs.resources.SimpleReloadInstance.lambda$prepareTasks$1(SimpleReloadInstance.java:51)",
];

pub fn function_load_failed(cause_description: &str) -> JavaThrowable {
    let mut cause_frames = FUNCTION_COMPILE_FRAMES.to_vec();
    cause_frames.extend(&FUNCTION_COMPLETION_FRAMES[5..]);
    cause_frames.extend(FORK_JOIN_TAIL);
    let mut frames = FUNCTION_COMPLETION_FRAMES.to_vec();
    frames.extend(FORK_JOIN_TAIL);
    let cause = JavaThrowable::new(cause_description, cause_frames);
    JavaThrowable::new(format!("java.util.concurrent.CompletionException: {cause_description}"), frames).with_cause(cause)
}

pub fn function_load_thrown(cause: JavaThrowable) -> JavaThrowable {
    let mut frames = FUNCTION_COMPLETION_FRAMES.to_vec();
    frames.extend(FORK_JOIN_TAIL);
    JavaThrowable::new(format!("java.util.concurrent.CompletionException: {}", cause.description), frames).with_cause(cause)
}

pub fn tag_decode_failed(context: TagLoadContext<'_>, message: &str) -> JavaThrowable {
    let mut frames = DATA_RESULT_GET_OR_THROW.to_vec();
    frames.extend(tag_load_frames(context, "net.minecraft.tags.TagLoader.load(TagLoader.java:61)"));
    JavaThrowable::new(format!("{ILLEGAL_STATE_EXCEPTION}: {message}"), frames)
}

pub fn tag_parse_failed(context: TagLoadContext<'_>, mut parse_error: JavaThrowable) -> JavaThrowable {
    append_frames(&mut parse_error, &tag_load_frames(context, "net.minecraft.tags.TagLoader.load(TagLoader.java:59)"));
    parse_error
}

pub fn registry_loading_failed(validation_stage: bool) -> JavaThrowable {
    let lambda = if validation_stage { LOAD_COMPLETION_VALIDATE_ERRORS } else { LOAD_COMPLETION_FREEZE_ERRORS };
    let completion = |first: &'static str| {
        let mut frames = vec![first, LOG_ERRORS, lambda];
        frames.extend(UNI_APPLY);
        frames.extend(FORK_JOIN_TAIL);
        frames
    };
    let illegal_state = JavaThrowable::new(format!("{ILLEGAL_STATE_EXCEPTION}: Failed to load registries due to errors"), completion(CREATE_REPORT_WITH_BRIEF_INFO_313));
    JavaThrowable::new("net.minecraft.ReportedException: Registry Loading", completion(CREATE_REPORT_WITH_BRIEF_INFO_329)).with_cause(illegal_state)
}

pub fn loot_table_validation_failed() -> JavaThrowable {
    let mut frames = LOOT_VALIDATION.to_vec();
    frames.extend(UNI_APPLY);
    frames.extend(FORK_JOIN_TAIL);
    JavaThrowable::new(format!("{ILLEGAL_STATE_EXCEPTION}: Found loot table validation problems"), frames)
}

const REGISTER_ELEMENTS: [&str; 17] = [
    "net.minecraft.core.MappedRegistry.register(MappedRegistry.java:105)",
    "net.minecraft.resources.RegistryLoadTask.lambda$registerElements$1(RegistryLoadTask.java:68)",
    "com.mojang.datafixers.util.Either$Left.ifLeft(Either.java:43)",
    "net.minecraft.resources.RegistryLoadTask.lambda$registerElements$0(RegistryLoadTask.java:68)",
    "java.base/java.util.stream.ForEachOps$ForEachOp$OfRef.accept(ForEachOps.java:186)",
    "java.base/java.util.stream.ReferencePipeline$3$1.accept(ReferencePipeline.java:214)",
    "java.base/java.util.stream.SortedOps$SizedRefSortingSink.end(SortedOps.java:357)",
    "java.base/java.util.stream.AbstractPipeline.copyInto(AbstractPipeline.java:571)",
    "java.base/java.util.stream.AbstractPipeline.wrapAndCopyInto(AbstractPipeline.java:560)",
    "java.base/java.util.stream.ForEachOps$ForEachOp.evaluateSequential(ForEachOps.java:153)",
    "java.base/java.util.stream.ForEachOps$ForEachOp$OfRef.evaluateSequential(ForEachOps.java:176)",
    "java.base/java.util.stream.AbstractPipeline.evaluate(AbstractPipeline.java:265)",
    "java.base/java.util.stream.ReferencePipeline.forEach(ReferencePipeline.java:632)",
    "net.minecraft.resources.RegistryLoadTask.registerElements(RegistryLoadTask.java:66)",
    "net.minecraft.resources.ResourceManagerRegistryLoadTask.lambda$load$3(ResourceManagerRegistryLoadTask.java:51)",
    "java.base/java.util.concurrent.CompletableFuture$UniAccept.tryFire(CompletableFuture.java:739)",
    "java.base/java.util.concurrent.CompletableFuture$Completion.exec(CompletableFuture.java:504)",
];

pub fn duplicate_value(value: &str) -> JavaThrowable {
    let mut frames = REGISTER_ELEMENTS.to_vec();
    frames.extend(FORK_JOIN_TAIL);
    JavaThrowable::new(format!("{ILLEGAL_STATE_EXCEPTION}: Adding duplicate value '{value}' to registry"), frames)
}

pub fn world_data_supplier_frames() -> Vec<&'static str> {
    let mut frames = vec!["net.minecraft.server.Main.lambda$main$1(Main.java:184)"];
    frames.extend(world_data_loader_frames());
    frames
}

pub fn world_data_loader_frames() -> Vec<&'static str> {
    let mut frames = vec![
        "net.minecraft.server.WorldLoader.lambda$load$2(WorldLoader.java:46)",
        "java.base/java.util.concurrent.CompletableFuture$UniCompose.tryFire(CompletableFuture.java:1171)",
        "java.base/java.util.concurrent.CompletableFuture$Completion.exec(CompletableFuture.java:504)",
    ];
    frames.extend(FORK_JOIN_TAIL);
    frames
}

pub fn execution_exception(cause: JavaThrowable) -> JavaThrowable {
    JavaThrowable::new(format!("java.util.concurrent.ExecutionException: {}", cause.description), EXECUTION_EXCEPTION_FRAMES.to_vec()).with_cause(cause)
}

pub fn completion_exception(cause: JavaThrowable) -> JavaThrowable {
    let mut frames = COMPLETION_EXCEPTION_FRAMES.to_vec();
    frames.extend(FORK_JOIN_TAIL);
    JavaThrowable::new(format!("java.util.concurrent.CompletionException: {}", cause.description), frames).with_cause(cause)
}

#[cfg(test)]
mod tests;
