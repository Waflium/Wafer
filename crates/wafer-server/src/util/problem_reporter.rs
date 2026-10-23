use std::cell::RefCell;
use std::rc::Rc;

use wafer_core::random::java_string_hash;

use super::java_collections::JavaHashMap;

#[derive(Clone, PartialEq, Debug)]
pub enum PathElement {
    Empty,
    CollectionReference { tag: String, registry: String },
    ElementReference { id: String, registry: String },
    Field(String),
    IndexedField(String, usize),
    Indexed(usize),
    MapEntry(String, String),
    RootElement { id: String, registry: String },
    RootField(String),
    Text(String),
}

impl PathElement {
    pub fn get(&self) -> String {
        match self {
            PathElement::Empty => String::new(),
            PathElement::Text(text) => text.clone(),
            PathElement::CollectionReference { tag, registry } => format!("->{{#{tag}@{registry}}}"),
            PathElement::ElementReference { id, registry } => format!("->{{{id}@{registry}}}"),
            PathElement::Field(name) => format!(".{name}"),
            PathElement::IndexedField(name, index) => format!(".{name}[{index}]"),
            PathElement::Indexed(index) => format!("[{index}]"),
            PathElement::MapEntry(name, key) => format!(".{name}[{key}]"),
            PathElement::RootElement { id, registry } => format!("{{{id}@{registry}}}"),
            PathElement::RootField(name) => name.clone(),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Problem {
    pub description: String,
    pub fatal: bool,
}

impl Problem {
    pub fn new(description: impl Into<String>) -> Problem {
        Problem { description: description.into(), fatal: false }
    }

    pub fn fatal(description: impl Into<String>) -> Problem {
        Problem { description: description.into(), fatal: true }
    }
}

struct Node {
    parent: Option<Rc<Node>>,
    element: PathElement,
}

#[derive(Clone)]
pub struct ProblemReporter {
    problems: Rc<RefCell<Vec<(Rc<Node>, Problem)>>>,
    node: Rc<Node>,
    root: Rc<Node>,
}

impl Default for ProblemReporter {
    fn default() -> ProblemReporter {
        ProblemReporter::collector(PathElement::Empty)
    }
}

struct ProblemTreeNode {
    element: PathElement,
    problems: Vec<Problem>,
    children: Vec<ProblemTreeNode>,
}

impl ProblemTreeNode {
    fn child(&mut self, element: &PathElement) -> &mut ProblemTreeNode {
        let index = match self.children.iter().position(|child| child.element == *element) {
            Some(index) => index,
            None => {
                self.children.push(ProblemTreeNode { element: element.clone(), problems: Vec::new(), children: Vec::new() });
                self.children.len() - 1
            }
        };
        &mut self.children[index]
    }

    fn get_lines(&self) -> Vec<String> {
        let problem_count = self.problems.len();
        let children_count = self.children.len();
        if problem_count == 0 && children_count == 0 {
            return Vec::new();
        }
        if problem_count == 0 && children_count == 1 {
            let mut lines = self.children[0].get_lines();
            lines[0] = self.element.get() + &lines[0];
            return lines;
        }
        if problem_count == 1 && children_count == 0 {
            return vec![format!("{}: {}", self.element.get(), self.problems[0].description)];
        }
        let mut lines: Vec<String> = self.children.iter().flat_map(ProblemTreeNode::get_lines).map(|line| format!("  {line}")).collect();
        for problem in &self.problems {
            lines.push(format!("  {}", problem.description));
        }
        lines.insert(0, format!("{}:", self.element.get()));
        lines
    }
}

impl ProblemReporter {
    pub fn collector(root: PathElement) -> ProblemReporter {
        let root = Rc::new(Node { parent: None, element: root });
        ProblemReporter { problems: Rc::default(), node: root.clone(), root }
    }

    pub fn for_child(&self, element: PathElement) -> ProblemReporter {
        let node = Rc::new(Node { parent: Some(self.node.clone()), element });
        ProblemReporter { problems: self.problems.clone(), node, root: self.root.clone() }
    }

    pub fn report(&self, problem: Problem) {
        let mut problems = self.problems.borrow_mut();
        if !problems.iter().any(|(node, existing)| Rc::ptr_eq(node, &self.node) && *existing == problem) {
            problems.push((self.node.clone(), problem));
        }
    }

    pub fn is_empty(&self) -> bool {
        self.problems.borrow().is_empty()
    }

    pub fn has_fatal_problems(&self) -> bool {
        self.problems.borrow().iter().any(|(_, problem)| problem.fatal)
    }

    fn path(node: &Rc<Node>, stop: Option<&Rc<Node>>) -> Vec<PathElement> {
        let mut elements = Vec::new();
        let mut current = Some(node);
        while let Some(node) = current {
            if stop.is_some_and(|stop| Rc::ptr_eq(stop, node)) {
                break;
            }
            elements.push(node.element.clone());
            current = node.parent.as_ref();
        }
        elements.reverse();
        elements
    }

    pub fn for_each(&self, mut output: impl FnMut(String, &Problem)) {
        for (node, problem) in self.problems.borrow().iter() {
            let path: String = ProblemReporter::path(node, None).iter().map(PathElement::get).collect();
            output(path, problem);
        }
    }

    pub fn get_report(&self) -> String {
        let mut grouped: JavaHashMap<Vec<String>> = JavaHashMap::new(java_string_hash);
        self.for_each(|path, problem| match grouped.get_mut(&path) {
            Some(problems) => problems.push(problem.description.clone()),
            None => {
                grouped.insert(path, vec![problem.description.clone()]);
            }
        });
        grouped.iter().map(|(path, problems)| format!(" at {path}: {}", problems.join("; "))).collect::<Vec<_>>().join("\n")
    }

    pub fn get_tree_report(&self) -> String {
        self.get_tree_report_rooted(self.root.element.clone())
    }

    pub fn get_tree_report_rooted(&self, root_element: PathElement) -> String {
        let mut root = ProblemTreeNode { element: root_element, problems: Vec::new(), children: Vec::new() };
        for (node, problem) in self.problems.borrow().iter() {
            let mut tree_node = &mut root;
            for element in ProblemReporter::path(node, Some(&self.root)) {
                tree_node = tree_node.child(&element);
            }
            tree_node.problems.push(problem.clone());
        }
        root.get_lines().join("\n")
    }
}
