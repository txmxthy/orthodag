//! Stable wire representations for public values.

use std::fmt;
use std::marker::PhantomData;

use ::serde::de::{Error as _, SeqAccess, Visitor};
use ::serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::MAX_ROUTE_POINTS;
use crate::drawing::{MAX_DRAWING_ITEMS, MAX_DRAWING_ROUTE_POINTS};
use crate::{Defect, Drawing, Graph, Node, Rect};

fn deserialize_bounded_vec<'de, D, T, const N: usize>(
    deserializer: D,
    name: &'static str,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct BoundedVecVisitor<T, const N: usize> {
        name: &'static str,
        marker: PhantomData<T>,
    }

    impl<'de, T, const N: usize> Visitor<'de> for BoundedVecVisitor<T, N>
    where
        T: Deserialize<'de>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "at most {N} {}", self.name)
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            if sequence.size_hint().is_some_and(|length| length > N) {
                return Err(A::Error::custom(format_args!(
                    "too many {}: limit is {N}",
                    self.name
                )));
            }
            let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(N));
            while let Some(value) = sequence.next_element()? {
                if values.len() == N {
                    return Err(A::Error::custom(format_args!(
                        "too many {}: limit is {N}",
                        self.name
                    )));
                }
                values.push(value);
            }
            Ok(values)
        }
    }

    deserializer.deserialize_seq(BoundedVecVisitor::<T, N> {
        name,
        marker: PhantomData,
    })
}

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
    #[serde(deserialize_with = "deserialize_route_points")]
    points: Vec<(i32, i32)>,
}

fn deserialize_route_points<'de, D>(deserializer: D) -> Result<Vec<(i32, i32)>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, _, MAX_ROUTE_POINTS>(deserializer, "route points")
}

#[derive(Serialize, Deserialize)]
struct DrawingWire {
    width: i32,
    height: i32,
    #[serde(deserialize_with = "deserialize_boxes")]
    boxes: Vec<BoxWire>,
    #[serde(deserialize_with = "deserialize_routes")]
    routes: Vec<RouteWire>,
}

fn deserialize_boxes<'de, D>(deserializer: D) -> Result<Vec<BoxWire>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, _, MAX_DRAWING_ITEMS>(deserializer, "drawing boxes")
}

fn deserialize_routes<'de, D>(deserializer: D) -> Result<Vec<RouteWire>, D::Error>
where
    D: Deserializer<'de>,
{
    struct RoutesVisitor;

    impl<'de> Visitor<'de> for RoutesVisitor {
        type Value = Vec<RouteWire>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "a bounded list of drawing routes")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            if sequence
                .size_hint()
                .is_some_and(|length| length > MAX_DRAWING_ITEMS)
            {
                return Err(A::Error::custom(format_args!(
                    "too many drawing routes: limit is {MAX_DRAWING_ITEMS}"
                )));
            }
            let mut routes =
                Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(MAX_DRAWING_ITEMS));
            let mut total_points = 0usize;
            while let Some(route) = sequence.next_element::<RouteWire>()? {
                if routes.len() == MAX_DRAWING_ITEMS {
                    return Err(A::Error::custom(format_args!(
                        "too many drawing routes: limit is {MAX_DRAWING_ITEMS}"
                    )));
                }
                total_points = total_points.saturating_add(route.points.len());
                if total_points > MAX_DRAWING_ROUTE_POINTS {
                    return Err(A::Error::custom(format_args!(
                        "too many drawing route points: limit is {MAX_DRAWING_ROUTE_POINTS}"
                    )));
                }
                routes.push(route);
            }
            Ok(routes)
        }
    }

    deserializer.deserialize_seq(RoutesVisitor)
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
