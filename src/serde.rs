//! Stable wire representations for public values.

use ::serde::de::Error as _;
use ::serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Defect, Drawing, Graph, Node, Rect};

#[derive(Serialize)]
struct NodeRef<'a> {
    label: &'a str,
    lines: &'a [String],
}

#[derive(Serialize)]
struct EdgeRef<'a> {
    from: usize,
    to: usize,
    tags: &'a [String],
}

#[derive(Serialize)]
struct GraphRef<'a> {
    nodes: Vec<NodeRef<'a>>,
    edges: Vec<EdgeRef<'a>>,
}

#[derive(Deserialize)]
struct NodeWire {
    label: String,
    #[serde(default)]
    lines: Vec<String>,
}

#[derive(Deserialize)]
struct EdgeWire {
    from: usize,
    to: usize,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct GraphWire {
    nodes: Vec<NodeWire>,
    #[serde(default)]
    edges: Vec<EdgeWire>,
}

impl Serialize for Graph {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        GraphRef {
            nodes: self
                .nodes()
                .iter()
                .map(|node| NodeRef {
                    label: node.label(),
                    lines: node.lines(),
                })
                .collect(),
            edges: self
                .edges()
                .iter()
                .map(|edge| EdgeRef {
                    from: edge.from().index(),
                    to: edge.to().index(),
                    tags: edge.tags(),
                })
                .collect(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Graph {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GraphWire::deserialize(deserializer)?;
        let mut graph = Graph::new();
        for node in wire.nodes {
            let mut held = Node::new(node.label);
            for line in node.lines {
                held = held.line(line);
            }
            graph.add_node(held);
        }
        for edge in wire.edges {
            let from = graph.node_id_at(edge.from).ok_or_else(|| {
                D::Error::custom(format_args!("node index {} is out of range", edge.from))
            })?;
            let to = graph.node_id_at(edge.to).ok_or_else(|| {
                D::Error::custom(format_args!("node index {} is out of range", edge.to))
            })?;
            graph
                .add_tagged_edge(from, to, edge.tags)
                .map_err(D::Error::custom)?;
        }
        graph.validate().map_err(D::Error::custom)?;
        Ok(graph)
    }
}

#[derive(Serialize, Deserialize)]
struct BoxWire {
    node: usize,
    column: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

#[derive(Serialize, Deserialize)]
struct RouteWire {
    edge: usize,
    points: Vec<(i32, i32)>,
}

#[derive(Serialize, Deserialize)]
struct DrawingWire {
    width: i32,
    height: i32,
    boxes: Vec<BoxWire>,
    routes: Vec<RouteWire>,
}

impl Serialize for Drawing {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let (width, height) = self.size();
        DrawingWire {
            width,
            height,
            boxes: self
                .boxes()
                .map(|boxed| BoxWire {
                    node: boxed.node.index(),
                    column: boxed.column,
                    x: boxed.rect.x,
                    y: boxed.rect.y,
                    w: boxed.rect.w,
                    h: boxed.rect.h,
                })
                .collect(),
            routes: self
                .routes()
                .map(|route| RouteWire {
                    edge: route.edge.index(),
                    points: route.points.to_vec(),
                })
                .collect(),
        }
        .serialize(serializer)
    }
}

impl Drawing {
    /// Deserializes a drawing and binds its node and edge indices to `graph`.
    ///
    /// Geometry and graph membership are validated before the drawing is
    /// returned. A graph is required because identifiers deliberately carry
    /// process-local graph identity, which is not part of the wire format.
    ///
    /// # Errors
    ///
    /// Returns the deserializer's error for malformed data, invalid geometry,
    /// or a node or edge index that does not exist in `graph`.
    pub fn deserialize_with<'de, D>(graph: &Graph, deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DrawingWire::deserialize(deserializer)?;
        let mut drawing = Self::new(wire.width, wire.height).map_err(D::Error::custom)?;
        for boxed in wire.boxes {
            let node = graph.node_id_at(boxed.node).ok_or_else(|| {
                D::Error::custom(format_args!("node index {} is out of range", boxed.node))
            })?;
            drawing
                .boxed(
                    node,
                    boxed.column,
                    Rect::new(boxed.x, boxed.y, boxed.w, boxed.h),
                )
                .map_err(D::Error::custom)?;
        }
        for route in wire.routes {
            let edge = graph.edge_id_at(route.edge).ok_or_else(|| {
                D::Error::custom(format_args!("edge index {} is out of range", route.edge))
            })?;
            drawing
                .route(edge, route.points)
                .map_err(D::Error::custom)?;
        }
        drawing.validate(graph).map_err(D::Error::custom)?;
        Ok(drawing)
    }
}

#[derive(Serialize, Deserialize)]
struct DefectWire {
    at: (i32, i32),
    fault: crate::Fault,
    edges: (usize, usize),
}

impl Serialize for Defect {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DefectWire {
            at: self.at,
            fault: self.fault,
            edges: (self.edges.0.index(), self.edges.1.index()),
        }
        .serialize(serializer)
    }
}

impl Defect {
    /// Deserializes a defect and binds its edge indices to `graph`.
    ///
    /// # Errors
    ///
    /// Returns the deserializer's error for malformed data or an edge index
    /// that does not exist in `graph`.
    pub fn deserialize_with<'de, D>(graph: &Graph, deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DefectWire::deserialize(deserializer)?;
        let edge = |index| {
            graph
                .edge_id_at(index)
                .ok_or_else(|| D::Error::custom(format_args!("edge index {index} is out of range")))
        };
        Ok(Self {
            at: wire.at,
            fault: wire.fault,
            edges: (edge(wire.edges.0)?, edge(wire.edges.1)?),
        })
    }
}
