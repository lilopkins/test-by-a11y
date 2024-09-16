use atspi::{
    proxy::{
        accessible::{AccessibleProxy, ObjectRefExt},
        action::ActionProxy,
    },
    zbus::{names::BusName, proxy::CacheProperties},
    AccessibilityConnection, Interface, Role,
};
use futures::future::try_join_all;

use crate::prelude::*;

const PROXY_DESTINATION: &str = "org.a11y.atspi.Registry";
const PROXY_INTERFACE: &str = "org.a11y.atspi.Accessible";

#[derive(Clone, Debug)]
struct TreeNode {
    destination: BusName<'static>,
    path: String,

    accessible_id: Option<String>,
    name: Option<String>,
    _role: Role,
    children: Vec<TreeNode>,
}

struct NodeDetails {
    destination: BusName<'static>,
    path: String,

    accessible_id: Option<String>,
    name: Option<String>,
    role: Role,
}

impl TreeNode {
    async fn from_accessible_proxy(ap: AccessibleProxy<'_>) -> atspi::Result<Self> {
        let connection = ap.inner().connection().clone();
        // Contains the processed `TreeNode`'s.
        let mut nodes: Vec<TreeNode> = Vec::new();

        // Contains the `AccessibleProxy` yet to be processed.
        let mut stack: Vec<AccessibleProxy> = vec![ap];

        // If the stack has an `AccessibleProxy`, we take the last.
        while let Some(ap) = stack.pop() {
            // Prevent obects with huge child counts from stalling the program.
            if ap.child_count().await? > 65536 {
                continue;
            }

            let child_objects = ap.get_children().await?;
            let mut children_proxies = try_join_all(
                child_objects
                    .into_iter()
                    .map(|child| child.into_accessible_proxy(&connection)),
            )
            .await?;

            log::trace!(
                "{} proxies to get data from as a child of {}",
                children_proxies.len(),
                ap.name().await?
            );
            let details = try_join_all(
                children_proxies
                    .iter()
                    .map(|child| Self::get_node_details(child)),
            )
            .await?;
            stack.append(&mut children_proxies);

            let children = details
                .into_iter()
                .map(|details| TreeNode {
                    destination: details.destination,
                    path: details.path,
                    accessible_id: details.accessible_id,
                    name: details.name,
                    _role: details.role,
                    children: Vec::new(),
                })
                .collect::<Vec<_>>();

            let NodeDetails {
                destination,
                path,
                accessible_id,
                name,
                role,
            } = Self::get_node_details(&ap).await?;
            nodes.push(TreeNode {
                destination,
                path,
                accessible_id,
                name,
                _role: role,
                children,
            });
        }

        let mut fold_stack: Vec<TreeNode> = Vec::with_capacity(nodes.len());

        while let Some(mut node) = nodes.pop() {
            if node.children.is_empty() {
                fold_stack.push(node);
                continue;
            }

            // If the node has children, we fold in the children from 'fold_stack'.
            // There may be more on 'fold_stack' than the node requires.
            let begin = fold_stack.len().saturating_sub(node.children.len());
            node.children = fold_stack.split_off(begin);
            fold_stack.push(node);
        }

        fold_stack
            .pop()
            .ok_or(atspi::AtspiError::Owned("No root node built".to_string()))
    }

    async fn get_node_details(node: &AccessibleProxy<'_>) -> atspi::Result<NodeDetails> {
        Ok(NodeDetails {
            destination: node.inner().destination().clone().into_owned(),
            path: node.inner().path().as_str().to_string(),
            accessible_id: node.accessible_id().await.ok(),
            name: node.name().await.ok(),
            role: node.get_role().await?,
        })
    }

    fn bfs(&self, by: By) -> Option<TreeNode> {
        // Check match
        match &by {
            By::Tag(tag) => {
                if self
                    .accessible_id
                    .as_ref()
                    .map(|t| t == tag)
                    .unwrap_or(false)
                {
                    return Some(self.clone());
                }
            }
            By::Text(text) => {
                if self.name.as_ref().map(|t| t == text).unwrap_or(false) {
                    return Some(self.clone());
                }
            }
        }
        // Check children
        for child in &self.children {
            if let Some(node) = child.bfs(by.clone()) {
                return Some(node);
            }
        }
        None
    }
}

/// An error from the ATSPI test interface.
#[derive(thiserror::Error, Debug)]
pub enum TestByATSPIError {
    /// Cannot find the application you specified to connect to.
    #[error("Cannot find the application you specified to connect to.")]
    CannotFindApplication,

    /// An ATSPI error occurred.
    #[error("ATSPI error: {0}")]
    Atspi(#[from] atspi::AtspiError),

    /// A ZBus error occurred.
    #[error("ZBus error: {0}")]
    Zbus(#[from] atspi::zbus::Error),

    /// The target does not support this kind of interaction
    #[error("The target you have provided does not support that kind of interaction")]
    CannotPerformInteractionOnTarget,

    /// The action cannot be found on this node
    #[error("The action cannot be found on this node")]
    CannotFindAction,
}

/// [`TestByA11y`] implemented for the Linux ATSPI accessibility API.
pub struct TestByATSPI<'p> {
    atspi: AccessibilityConnection,
    root_proxy: AccessibleProxy<'p>,
}
impl<'p> TestByATSPI<'p> {
    async fn connect_impl(
        app_name: <TestByATSPI<'p> as TestByA11y>::Init,
    ) -> Result<Self, <TestByATSPI<'p> as TestByA11y>::Error> {
        log::debug!("Establishing ATSPI connection");
        let atspi = AccessibilityConnection::new().await?;
        log::trace!("Getting root proxy");
        let proxy = AccessibleProxy::builder(atspi.connection())
            .destination(PROXY_DESTINATION)?
            .path("/org/a11y/atspi/accessible/root")?
            .interface(PROXY_INTERFACE)?
            .cache_properties(CacheProperties::No)
            .build()
            .await?;

        // Find application
        log::trace!("Finding application. Searching for: {app_name:?}");
        let mut potential_matches = vec![];
        let conn = proxy.inner().connection();
        for child in proxy.get_children().await? {
            let ap = child.into_accessible_proxy(conn).await?;
            if ap.get_role().await? == atspi::Role::Application {
                let name = ap.name().await?;
                log::trace!("Found app with name {name:?}");
                if name == app_name {
                    log::trace!("Name matches");
                    potential_matches.push(ap);
                }
            }
        }

        if potential_matches.is_empty() {
            log::debug!("No app match");
            return Err(TestByATSPIError::CannotFindApplication);
        }
        if potential_matches.len() > 1 {
            log::debug!("Too many matches");
            return Err(TestByATSPIError::CannotFindApplication);
        }
        let app = potential_matches[0].clone();
        let dest = app.inner().destination().clone().into_owned();
        let path = app.inner().path().as_str().to_string();
        let root_proxy = AccessibleProxy::builder(atspi.connection())
            .destination(dest)?
            .path(path)?
            .interface(PROXY_INTERFACE)?
            .cache_properties(CacheProperties::No)
            .build()
            .await?;
        log::debug!("Root: {root_proxy:?}");
        Ok(TestByATSPI { atspi, root_proxy })
    }

    async fn find_impl(
        &mut self,
        by: By,
    ) -> Result<Option<<TestByATSPI<'p> as TestByA11y>::Node>, <TestByATSPI<'p> as TestByA11y>::Error>
    {
        log::trace!("Searching for {by:?}");
        // Build tree
        let tree = self.build_tree().await?;
        // Search tree
        let node = tree.bfs(by);
        if let Some(node) = node {
            log::trace!("Found node, building new proxy");
            return Ok(Some(
                AccessibleProxy::builder(self.atspi.connection())
                    .destination(node.destination.clone())?
                    .path(node.path.clone())?
                    .interface(PROXY_INTERFACE)?
                    .cache_properties(CacheProperties::No)
                    .build()
                    .await?,
            ));
        }
        Ok(None)
    }

    async fn interact_impl(
        &mut self,
        node: &<TestByATSPI<'p> as TestByA11y>::Node,
        interaction: Interaction,
    ) -> Result<(), <TestByATSPI<'p> as TestByA11y>::Error> {
        log::debug!("Interaction {interaction:?} on {}", node.name().await?);
        let interfaces = node.get_interfaces().await.unwrap_or_default();
        match interaction {
            Interaction::Click => {
                // TODO Trigger click
                if !interfaces.contains(Interface::Action) {
                    return Err(TestByATSPIError::CannotPerformInteractionOnTarget);
                }
                let action_proxy = ActionProxy::builder(self.atspi.connection())
                    .destination(node.inner().destination().clone())?
                    .path(node.inner().path().clone())?
                    .interface("org.a11y.atspi.Action")?
                    .cache_properties(CacheProperties::No)
                    .build()
                    .await?;
                let actions = action_proxy.get_actions().await?;
                log::trace!("Actions: {actions:?}");
                for (idx, action) in actions.iter().enumerate() {
                    if action.name.to_lowercase().contains("click")
                        || action.description.to_lowercase().contains("click")
                    {
                        action_proxy.do_action(idx as i32).await?;
                        return Ok(());
                    }
                }
                Err(TestByATSPIError::CannotFindAction)
            }
        }
    }

    async fn get_text_impl(
        &mut self,
        node: &<TestByATSPI<'p> as TestByA11y>::Node,
    ) -> Result<String, <TestByATSPI<'p> as TestByA11y>::Error> {
        Ok(node.name().await?)
    }

    async fn build_tree(&self) -> Result<TreeNode, atspi::AtspiError> {
        log::trace!("Building tree");
        TreeNode::from_accessible_proxy(self.root_proxy.clone()).await
    }
}

impl<'p> TestByA11y for TestByATSPI<'p> {
    /// The program name you will use for the root of the accessibility instance.
    type Init = String;
    type Error = TestByATSPIError;
    type Node = AccessibleProxy<'p>;

    fn connect(init: Self::Init) -> Result<Self, Self::Error> {
        futures::executor::block_on(Self::connect_impl(init))
    }

    fn find(&mut self, by: By) -> Result<Option<Self::Node>, Self::Error> {
        log::trace!("find(by: {by:?})");
        let r = futures::executor::block_on(self.find_impl(by.clone()));
        log::trace!("find(by: {by:?}) = {r:?}");
        r
    }

    fn interact(&mut self, node: &Self::Node, interaction: Interaction) -> Result<(), Self::Error> {
        log::trace!("interact(node: {node:?}, interaction: {interaction:?})");
        let r = futures::executor::block_on(self.interact_impl(node, interaction));
        log::trace!("interact(node: ..., interaction: {interaction:?}) = {r:?}");
        r
    }

    fn get_text(&mut self, node: &Self::Node) -> Result<String, Self::Error> {
        log::trace!("get_text(node: {node:?})");
        let r = futures::executor::block_on(self.get_text_impl(node));
        log::trace!("get_text(node: ...) = {r:?}");
        r
    }
}
