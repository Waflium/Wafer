use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::commands::create_compilation_context;
use crate::commands::functions::CommandFunction;
use crate::server::packs::resources::MultiPackResourceManager;
use crate::resources::registry_load_task::TagLoadContext;
use crate::tags::{TagLoader, identifier_order};
use crate::util::java_collections::{JavaHashMap, identifier_hash};
use crate::util::java_throwable::JavaThrowable;

const FUNCTION_DIRECTORY: &str = "function";
const FUNCTION_EXTENSION: &str = ".mcfunction";
const TAG_DIRECTORY: &str = "tags/function";

static NEXT_WORKER: AtomicUsize = AtomicUsize::new(2);

pub struct ServerFunctionLibrary {
    functions: JavaHashMap<Arc<CommandFunction>>,
    tags: JavaHashMap<Vec<Arc<CommandFunction>>>,
}

impl Default for ServerFunctionLibrary {
    fn default() -> ServerFunctionLibrary {
        ServerFunctionLibrary { functions: JavaHashMap::new(identifier_hash), tags: JavaHashMap::new(identifier_hash) }
    }
}

pub(crate) fn worker_name() -> String {
    format!("Worker-Main-{}", NEXT_WORKER.fetch_add(1, Ordering::Relaxed))
}

fn read_lines(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = Vec::new();
    let mut rest = text.as_ref();
    while !rest.is_empty() {
        match rest.find(['\n', '\r']) {
            Some(end) => {
                lines.push(rest[..end].to_string());
                let skip = if rest[end..].starts_with("\r\n") { 2 } else { 1 };
                rest = &rest[end + skip..];
            }
            None => {
                lines.push(rest.to_string());
                break;
            }
        }
    }
    lines
}

impl ServerFunctionLibrary {
    pub fn get(&self, id: &str) -> Option<Arc<CommandFunction>> {
        self.functions.get(id).cloned()
    }

    pub fn get_tag(&self, id: &str) -> &[Arc<CommandFunction>] {
        self.tags.get(id).map_or(&[], Vec::as_slice)
    }

    pub fn function_names(&self) -> impl Iterator<Item = &str> {
        self.functions.iter().map(|(id, _)| id)
    }

    pub fn available_tags(&self) -> impl Iterator<Item = &str> {
        self.tags.iter().map(|(id, _)| id)
    }

    pub fn reload(resource_manager: &MultiPackResourceManager, function_compilation_permission: i32) -> Result<ServerFunctionLibrary, JavaThrowable> {
        let tags_loader = TagLoader::with_context(TAG_DIRECTORY, Some(TagLoadContext::FunctionLibrary));
        let mut functions_to_load: Vec<_> = resource_manager.list_resources(FUNCTION_DIRECTORY, FUNCTION_EXTENSION).into_iter().collect();
        identifier_order(&mut functions_to_load);
        let mut result: JavaHashMap<usize> = JavaHashMap::new(identifier_hash);
        let mut sources = Vec::with_capacity(functions_to_load.len());
        for (resource_id, resource) in functions_to_load {
            let (namespace, path) = resource_id.split_once(':').unwrap();
            let id = format!("{namespace}:{}", &path[FUNCTION_DIRECTORY.len() + 1..path.len() - FUNCTION_EXTENSION.len()]);
            result.insert(id.clone(), sources.len());
            sources.push((id, resource));
        }
        let compilation_context = create_compilation_context(function_compilation_permission);
        let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).min(sources.len()).max(1);
        let next = AtomicUsize::new(0);
        let mut compiled: Vec<Option<Result<CommandFunction, crate::util::java_throwable::JavaThrowable>>> =(0..sources.len()).map(|_| None).collect();
        let tags = std::thread::scope(|scope| {
            let tags = std::thread::Builder::new().name(worker_name()).spawn_scoped(scope, || crate::util::catch_throwable(|| tags_loader.load(resource_manager))).expect("worker thread");
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    let (sources, next, compilation_context) = (&sources, &next, &compilation_context);
                    std::thread::Builder::new()
                        .name(worker_name())
                        .stack_size(64 << 20)
                        .spawn_scoped(scope, move || {
                            crate::util::catch_throwable(|| {
                                let mut done = Vec::new();
                                loop {
                                    let index = next.fetch_add(1, Ordering::Relaxed);
                                    let Some((id, resource)) = sources.get(index) else { break };
                                    let function = crate::util::catch_throwable(|| {
                                        resource.supplier.read().map_err(|e| format!("java.io.IOException: {e}")).and_then(|bytes| {
                                            CommandFunction::from_lines(id, compilation_context, &read_lines(&bytes))
                                                .map_err(|e| format!("java.lang.IllegalArgumentException: {e}"))
                                        })
                                    });
                                    let function = match function {
                                        Ok(function) => function.map_err(|exception| crate::resources::registry_load_task::function_load_failed(&exception)),
                                        Err(thrown) => Err(crate::resources::registry_load_task::function_load_thrown(thrown)),
                                    };
                                    done.push((index, function));
                                }
                                done
                            })
                        })
                        .expect("worker thread")
                })
                .collect();
            let mut failure = None;
            for handle in handles {
                match handle.join().unwrap_or_else(|payload| Err(crate::crash_report::throwable_of(&payload))) {
                    Ok(done) => {
                        for (index, function) in done {
                            compiled[index] = Some(function);
                        }
                    }
                    Err(throwable) => failure = failure.or(Some(throwable)),
                }
            }
            let tags = tags.join().unwrap_or_else(|payload| Err(crate::crash_report::throwable_of(&payload)));
            match failure {
                Some(throwable) => Err(throwable),
                None => tags,
            }
        })?;
        let mut functions: JavaHashMap<Arc<CommandFunction>> = JavaHashMap::new(identifier_hash);
        for (id, &index) in result.iter() {
            match compiled[index].take().unwrap() {
                Ok(function) => {
                    functions.insert(id.to_string(), Arc::new(function));
                }
                Err(exception) => error!("Failed to load function {id}\n{}", exception.print_stack_trace().trim_end()),
            }
        }
        let tags = tags_loader.build(&tags, &|id, _| functions.get(id).cloned(), &Arc::ptr_eq);
        Ok(ServerFunctionLibrary { functions, tags })
    }
}
