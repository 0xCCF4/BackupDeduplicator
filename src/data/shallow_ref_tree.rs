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
    let value = *counter;
    drop(counter);
    value
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

/// A tree data structure.
pub struct ShallowRefTree<Node> {
    nodes: BTreeMap<NodeId, TreeNode<Node>>,
    root_id: NodeId,
    parent_ref: BTreeMap<NodeId, NodeId>,
    child_ref: BTreeMap<NodeId, BTreeSet<NodeId>>,
}

impl<Node> ShallowRefTree<Node> {
    /// Create a new tree with the given root node.
    ///
    /// # Arguments
    /// * `root` - The root node of the tree.
    ///
    /// # Returns
    /// A new tree with the given root node.
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

    /// Returns the root node's id.
    ///
    /// # Returns
    /// The root node's id.
    pub fn root_id(&self) -> NodeId {
        self.root_id
    }

    /// Returns the root node as reference
    ///
    /// # Returns
    /// The root node.
    pub fn root(&self) -> &TreeNode<Node> {
        self.nodes.get(&self.root_id).expect("Root node must exist")
    }

    /// Returns the root node as mutable reference
    ///
    /// # Returns
    /// The root node.
    pub fn root_mut(&mut self) -> &mut TreeNode<Node> {
        self.nodes
            .get_mut(&self.root_id)
            .expect("Root node must exist")
    }

    /// Check if the tree contains the given node. Comparison is achieved via the `NodeId`
    ///
    /// # Arguments
    /// * `node` - The node to check for.
    ///
    /// # Returns
    /// `true` if the tree contains the given node id
    pub fn contains<I: Into<NodeId>>(&self, node: I) -> bool {
        let id = node.into();
        self.nodes.contains_key(&id)
    }

    /// Add a child node to the given parent node.
    ///
    /// # Arguments
    /// * `child` the node to add to the tree
    /// * `parent`the node to add the node as child to
    ///
    /// # Returns
    /// * the id of the newly added node
    /// * `None` if the parent was not found and the node was not added to the tree
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

    /// Query the parent node id for a given node
    ///
    /// # Arguments
    /// * `node` the node to search for the parent node
    ///
    /// # Returns
    /// * `Some(NodeId)` if the parent was found
    /// * `None` if the node is not part of the tree or if it is the root node
    pub fn parent<I: Into<NodeId>>(&self, node: I) -> Option<NodeId> {
        let id = node.into();
        self.parent_ref.get(&id).map(|node| node.clone())
    }

    /// Query the parent node for a given node and return a reference to its contents.
    ///
    /// # Arguments
    /// * `node` the node to search for the parent node
    ///
    /// # Returns
    /// * `Some(&TreeNode<Node>)` if the parent was found
    /// * `None` if the node is not part of the tree or if it is the root node
    pub fn parent_ref<I: Into<NodeId>>(&self, node: I) -> Option<&TreeNode<Node>> {
        let parent = self.parent(node)?;
        self.nodes.get(&parent)
    }

    /// Query the parent node for a given node and return a mutable reference to its contents.
    ///
    /// # Arguments
    /// * `node` the node to search for the parent node
    ///
    /// # Returns
    /// * `Some(&mut TreeNode<Node>)` if the parent was found
    /// * `None` if the node is not part of the tree or if it is the root node
    pub fn parent_mut<I: Into<NodeId>>(&mut self, node: I) -> Option<&mut TreeNode<Node>> {
        let parent = self.parent(node)?;
        self.nodes.get_mut(&parent)
    }

    /// Query the children of the given node
    ///
    /// # Arguments
    /// * `node` the node to search for children
    ///
    /// # Returns
    /// * `Some(Vec<NodeId>)` the list of children node ids
    /// * `None` if the node is not part of the tree
    pub fn children<I: Into<NodeId>>(&self, node: I) -> Option<Vec<NodeId>> {
        let id = node.into();
        self.child_ref
            .get(&id)
            .map(|node| node.iter().cloned().collect::<Vec<_>>())
    }

    /// Query the children of the given node and return references to their contents.
    ///
    /// # Arguments
    /// * `node` the node to search for children
    ///
    /// # Returns
    /// * `Some(Vec<&TreeNode<Node>>)` the list of references to the children nodes
    /// * `None` if the node is not part of the tree
    pub fn children_ref<I: Into<NodeId>>(&self, node: I) -> Option<Vec<&TreeNode<Node>>> {
        let children = self.children(node)?;
        children.into_iter().map(|id| self.nodes.get(&id)).collect()
    }

    /// Query the children of the given node and return mutable references to their contents.
    ///
    /// # Arguments
    /// * `node` the node to search for children
    ///
    /// # Returns
    /// * `Some(Vec<&mut TreeNode<Node>>)` the list of mutable references to the children nodes
    /// * `None` if the node is not part of the tree
    ///
    /// # Safety
    /// This function exposes the mutable references and captures the &mut self reference until
    /// the returned `TreeMutList` is dropped. This ensures that no other mutable references
    /// to the tree can be created while the mutable references to the children are held.
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

    /// Returns a reference to the node with the given id.
    ///
    /// # Arguments
    /// * `node` the node id to search for
    ///
    /// # Returns
    /// * `Some(&TreeNode<Node>)` if the node was found in the tree
    /// * `None` if the node was not found in the tree
    pub fn node<I: Into<NodeId>>(&self, node: I) -> Option<&TreeNode<Node>> {
        self.nodes.get(&node.into())
    }

    /// Returns a mutable reference to the node with the given id.
    ///
    /// # Arguments
    /// * `node` the node id to search for
    ///
    /// # Returns
    /// * `Some(&mut TreeNode<Node>)` if the node was found in the tree
    /// * `None` if the node was not found in the tree
    pub fn node_mut<I: Into<NodeId>>(&mut self, node: I) -> Option<&mut TreeNode<Node>> {
        self.nodes.get_mut(&node.into())
    }

    /// Remove a node and all its children (recursively) from the tree.
    ///
    /// # Arguments
    /// * `node` the node to remove
    pub fn remove_children<I: Into<NodeId>>(&mut self, node: I) {
        let mut remove_queue = VecDeque::new();
        let parent_node = node.into();
        remove_queue.push_back(parent_node);

        while let Some(node_id) = remove_queue.pop_front() {
            if node_id != parent_node {
                self.nodes.remove(&node_id);
            }
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
    /// Export the tree to a dotfile for visualization purposes.
    ///
    /// # Arguments
    /// * `stream` - The stream to write the dotfile contents to.
    ///
    /// # Errors
    /// * If writing to the stream fails.
    pub fn to_dotfile<W: Write>(&self, stream: &mut W) -> Result<(), std::io::Error> {
        writeln!(stream, "digraph tree {{")?;

        for (node_id, node) in self.nodes.iter() {
            assert_eq!(*node_id, node.id, "ids must always be equal");
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

/// When instantiating a tree with nodes of this trait, the tree can be exported to a dotfile
pub trait DebugGraph {
    /// The color type. Must implement the `Display` trait
    type Color;
    /// The label type. Must implement the `Display` trait
    type Label;
    /// Returns the color of that node for visualization purposes
    fn debug_color(&self) -> Option<Self::Color>;
    /// Returns the label of that node for visualization purposes
    fn label(&self) -> Option<Self::Label>;
}

/// List of mutable tree nodes. When querying for mutable children, this struct is returned.
/// It holds a reference to the parent tree to ensure that the parent tree is not modified
/// while the mutable references are held.
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
