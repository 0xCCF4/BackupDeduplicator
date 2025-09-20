use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::{Debug, Display};
use std::io::Write;
use std::ops::Deref;
use std::sync::Mutex;

pub type NodeId = u64;

static NODE_COUNTER: Mutex<NodeId> = Mutex::new(0);

fn new_node_id() -> NodeId {
    let mut counter = NODE_COUNTER.lock().expect("Failed to lock job counter");
    *counter += 1;
    *counter
}

#[derive(Debug)]
pub struct TreeNode<Node> {
    id: NodeId,
    pub content: Node,
}

impl<Node> TreeNode<Node> {
    pub fn new(content: Node) -> TreeNode<Node> {
        Self {
            id: new_node_id(),
            content,
        }
    }
    pub fn id(&self) -> NodeId {
        self.id
    }
}

impl<Node> From<Node> for TreeNode<Node> {
    fn from(node: Node) -> Self {
        Self::new(node)
    }
}

impl<Node> From<TreeNode<Node>> for NodeId {
    fn from(tree_node: TreeNode<Node>) -> Self {
        tree_node.id
    }
}

impl<Node> From<&TreeNode<Node>> for NodeId {
    fn from(tree_node: &TreeNode<Node>) -> Self {
        tree_node.id
    }
}

impl<Node> PartialEq for TreeNode<Node> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<Node> Eq for TreeNode<Node> {}

impl<Node> PartialOrd for TreeNode<Node> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl<Node> Ord for TreeNode<Node> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

pub struct ShallowRefTree<Node> {
    pub nodes: BTreeMap<NodeId, TreeNode<Node>>,
    pub root_id: NodeId,
    pub parent_ref: BTreeMap<NodeId, NodeId>,
    pub child_ref: BTreeMap<NodeId, BTreeSet<NodeId>>,
}

impl<Node> ShallowRefTree<Node> {
    pub fn new<I: Into<TreeNode<Node>>>(root: I) -> Self {
        let root = root.into();
        let mut nodes = BTreeMap::new();
        let root_id = root.id;
        nodes.insert(root_id, root);
        let mut child_ref = BTreeMap::new();
        child_ref.insert(root_id, BTreeSet::new());
        Self {
            nodes,
            root_id,
            parent_ref: BTreeMap::new(),
            child_ref,
        }
    }

    pub fn contains<I: Into<NodeId>>(&self, node: I) -> bool {
        let id = node.into();
        self.nodes.contains_key(&id)
    }

    pub fn add_child<I: Into<TreeNode<Node>>>(
        &mut self,
        parent: NodeId,
        child: I,
    ) -> Option<NodeId> {
        if !self.contains(parent) {
            return None;
        }

        let node = child.into();
        let id = node.id;

        self.nodes.insert(node.id, node);
        self.parent_ref.insert(id, parent);
        self.child_ref.insert(id, BTreeSet::new());

        if let Some(parent_child_ref) = self.child_ref.get_mut(&parent) {
            parent_child_ref.insert(id);
        }

        Some(id)
    }

    pub fn parent<I: Into<NodeId>>(&self, node: I) -> Option<NodeId> {
        let id = node.into();
        self.parent_ref.get(&id).map(|node| node.clone())
    }

    pub fn parent_ref<I: Into<NodeId>>(&self, node: I) -> Option<&TreeNode<Node>> {
        let parent = self.parent(node)?;
        self.nodes.get(&parent)
    }

    pub fn parent_mut<I: Into<NodeId>>(&mut self, node: I) -> Option<&mut TreeNode<Node>> {
        let parent = self.parent(node)?;
        self.nodes.get_mut(&parent)
    }

    pub fn children<I: Into<NodeId>>(&self, node: I) -> Option<Vec<NodeId>> {
        let id = node.into();
        self.child_ref
            .get(&id)
            .map(|node| node.iter().cloned().collect::<Vec<_>>())
    }

    pub fn children_ref<I: Into<NodeId>>(&self, node: I) -> Option<Vec<&TreeNode<Node>>> {
        let children = self.children(node)?;
        children.into_iter().map(|id| self.nodes.get(&id)).collect()
    }

    #[allow(elided_named_lifetimes)]
    pub fn children_mut<'a, I: Into<NodeId>>(&'a mut self, node: I) -> Option<TreeMutList<Node>> {
        let children = self.children(node)?;
        let list = children
            .into_iter()
            .filter_map(|id| {
                (self.nodes.get_mut(&id)).map(|node| unsafe {
                    std::mem::transmute::<&mut TreeNode<Node>, &'a mut TreeNode<Node>>(node)
                })
            })
            .collect::<Vec<_>>();
        // safe since we capture &mut self and do not allow moving out any reference from TreeMutList
        Some(TreeMutList {
            list,
            _parent: self,
        })
    }

    pub fn node<I: Into<NodeId>>(&self, node: I) -> Option<&TreeNode<Node>> {
        self.nodes.get(&node.into())
    }
    pub fn node_mut<I: Into<NodeId>>(&mut self, node: I) -> Option<&mut TreeNode<Node>> {
        self.nodes.get_mut(&node.into())
    }
    pub fn remove_children<I: Into<NodeId>>(&mut self, node: I) {
        let mut remove_queue = VecDeque::new();
        remove_queue.push_back(node.into());

        while let Some(node_id) = remove_queue.pop_front() {
            self.nodes.remove(&node_id);
            if let Some(children) = self.children(node_id) {
                remove_queue.extend(children.into_iter());
            }
            self.child_ref.remove(&node_id);
            self.parent_ref.remove(&node_id);
        }
    }
}

impl<Color: Display, Label: Display, Node: DebugGraph<Color = Color, Label = Label>>
    ShallowRefTree<Node>
{
    pub fn to_dotfile<W: Write>(&self, stream: &mut W) -> Result<(), std::io::Error> {
        writeln!(stream, "digraph tree {{")?;

        for (node_id, node) in self.nodes.iter() {
            let label = node
                .content
                .label()
                .map(|x| x.to_string())
                .unwrap_or_default()
                .replace("\"", "'")
                .replace("\\", "");
            let limited_label = &label[..500.min(label.len())];
            let color = match node.content.debug_color() {
                None => "".to_string(),
                Some(color) => format!(
                    " style=filled color={}",
                    color.to_string().replace("\"", "'").replace("\\", "")
                ),
            };
            writeln!(
                stream,
                "\t\"N{node_id}\"[label=\"{node_id}:{limited_label}\"{color}];"
            )?;
        }

        for (node_id, children) in self.child_ref.iter() {
            for child_id in children.iter() {
                writeln!(stream, "\t\"N{node_id}\" -> \"N{child_id}\";")?;
            }
        }

        writeln!(stream, "}}")?;

        Ok(())
    }
}

pub trait DebugGraph {
    type Color;
    type Label;
    fn debug_color(&self) -> Option<Self::Color>;
    fn label(&self) -> Option<Self::Label>;
}

pub struct TreeMutList<'node, 'tree: 'node, Node> {
    list: Vec<&'node mut TreeNode<Node>>,
    _parent: &'tree mut ShallowRefTree<Node>,
}

impl<'node, 'tree, Node> Deref for TreeMutList<'node, 'tree, Node> {
    type Target = Vec<&'node mut TreeNode<Node>>;
    fn deref(&self) -> &Self::Target {
        &self.list
    }
}
