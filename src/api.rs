/// Inheritors of this trait are capable of testing by accessibility interfaces.
pub trait TestByA11y: Sized {
    /// The data type needed for initialisation of a connection to this interface.
    type Init;
    /// The error type this interface returns for it's processes.
    type Error;
    /// The type that contains a node for this interface.
    type Node;

    /// Create a new connection to this accessibility interface for a program.
    fn connect(init: Self::Init) -> Result<Self, Self::Error>;

    /// Find an node by the specified query.
    fn find(&mut self, by: By) -> Result<Option<Self::Node>, Self::Error>;

    /// Interact with a node.
    fn interact(&mut self, node: &Self::Node, interaction: Interaction) -> Result<(), Self::Error>;

    /// Get the text from a node.
    fn get_text(&mut self, node: &Self::Node) -> Result<String, Self::Error>;
}

/// Ways we can find nodes
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum By {
    /// By a tag that is machine-visible. On Linux, this is the accessibility ID.
    Tag(String),
    /// By some human readable text. This should match partially as well.
    Text(String),
}

/// Ways we can interact with nodes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Interaction {
    /// Click on the node.
    Click,
}
