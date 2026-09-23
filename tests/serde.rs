#![cfg(feature = "serde")]
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use orthodag::{Crossing, Defect, Drawing, Fault, Graph, Node, Options, Rect, Score, layout};

fn graph() -> Graph {
    let mut graph = Graph::new();
    let source = graph.add_node(Node::new("source").line("one"));
    let sink = graph.add_node(Node::new("sink"));
    graph
        .add_tagged_edge(source, sink, ["late", "odd"])
        .unwrap();
    graph
}

#[test]
fn a_graph_round_trips_without_serialising_process_identity() {
    let graph = graph();
    let json = serde_json::to_string(&graph).unwrap();

    assert_eq!(
        json,
        r#"{"nodes":[{"label":"source","lines":["one"]},{"label":"sink","lines":[]}],"edges":[{"from":0,"to":1,"tags":["late","odd"]}]}"#
    );
    let decoded: Graph = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, graph);
    assert_ne!(decoded.node_ids().next(), graph.node_ids().next());
}

#[test]
fn malformed_graph_references_are_rejected() {
    let json = r#"{"nodes":[{"label":"only","lines":[]}],"edges":[{"from":0,"to":2,"tags":[]}]}"#;

    let error = serde_json::from_str::<Graph>(json).unwrap_err();
    assert!(error.to_string().contains("node index 2"));
}

#[test]
fn options_and_scores_use_stable_snake_case_fields() {
    let options = Options::new().labels(true).crossings(Crossing::Bridge);
    assert_eq!(
        serde_json::to_string(&options).unwrap(),
        r#"{"labels":true,"crossings":"bridge","width":null,"box_width":null,"box_height":null}"#
    );
    assert_eq!(
        serde_json::from_str::<Options>(&serde_json::to_string(&options).unwrap()).unwrap(),
        options
    );

    let score = Score {
        cross_cells: 2,
        total: 2,
        ..Score::default()
    };
    assert_eq!(
        serde_json::from_str::<Score>(&serde_json::to_string(&score).unwrap()).unwrap(),
        score
    );
}

#[test]
fn options_and_scores_default_fields_missing_from_older_data() {
    assert_eq!(
        serde_json::from_str::<Options>("{}").unwrap(),
        Options::default()
    );
    assert_eq!(
        serde_json::from_str::<Options>(r#"{"labels":true}"#).unwrap(),
        Options::default().labels(true)
    );
    assert_eq!(
        serde_json::from_str::<Score>("{}").unwrap(),
        Score::default()
    );
}

#[test]
fn a_drawing_rebinds_to_and_validates_against_its_graph() {
    let original = graph();
    let drawing = layout(&original, Options::default());
    let json = serde_json::to_string(&drawing).unwrap();
    let decoded_graph: Graph =
        serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();
    let mut deserializer = serde_json::Deserializer::from_str(&json);
    let decoded = Drawing::deserialize_with(&decoded_graph, &mut deserializer).unwrap();

    assert_eq!(decoded.size(), drawing.size());
    let boxes = |drawing: &Drawing| {
        drawing
            .boxes()
            .map(|boxed| (boxed.node.index(), boxed.column, boxed.rect))
            .collect::<Vec<_>>()
    };
    assert_eq!(boxes(&decoded), boxes(&drawing));
    assert_eq!(
        decoded
            .routes()
            .map(|route| (route.edge.index(), route.points.to_vec(), route.heading))
            .collect::<Vec<_>>(),
        drawing
            .routes()
            .map(|route| (route.edge.index(), route.points.to_vec(), route.heading))
            .collect::<Vec<_>>()
    );
    decoded.validate(&decoded_graph).unwrap();
}

#[test]
fn invalid_drawing_geometry_and_unknown_ids_are_rejected_on_decode() {
    let graph = graph();
    let diagonal =
        r#"{"width":4,"height":4,"boxes":[],"routes":[{"edge":0,"points":[[0,0],[1,1]]}]}"#;
    let mut deserializer = serde_json::Deserializer::from_str(diagonal);
    assert!(Drawing::deserialize_with(&graph, &mut deserializer).is_err());

    let unknown = r#"{"width":4,"height":4,"boxes":[{"node":9,"column":0,"x":0,"y":0,"w":2,"h":2}],"routes":[]}"#;
    let mut deserializer = serde_json::Deserializer::from_str(unknown);
    let error = Drawing::deserialize_with(&graph, &mut deserializer).unwrap_err();
    assert!(error.to_string().contains("node index 9"));
}

#[test]
fn oversized_route_data_is_rejected_during_decode() {
    let graph = graph();
    let points = std::iter::repeat_n("[0,0]", orthodag::MAX_ROUTE_POINTS + 1)
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        r#"{{"width":4,"height":4,"boxes":[],"routes":[{{"edge":0,"points":[{points}]}}]}}"#
    );
    let mut deserializer = serde_json::Deserializer::from_str(&json);
    let error = Drawing::deserialize_with(&graph, &mut deserializer).unwrap_err();

    assert!(error.to_string().contains("route points"));
    assert!(error.to_string().contains("limit"));
}

#[test]
fn aggregate_route_data_is_bounded_during_decode() {
    let graph = graph();
    let points = std::iter::repeat_n("[0,0]", orthodag::MAX_ROUTE_POINTS)
        .collect::<Vec<_>>()
        .join(",");
    let route = format!(r#"{{"edge":0,"points":[{points}]}}"#);
    let routes = std::iter::repeat_n(route.as_str(), 65)
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(r#"{{"width":4,"height":4,"boxes":[],"routes":[{routes}]}}"#);
    let mut deserializer = serde_json::Deserializer::from_str(&json);
    let error = Drawing::deserialize_with(&graph, &mut deserializer).unwrap_err();

    assert!(error.to_string().contains("drawing route points"));
    assert!(error.to_string().contains("limit"));
}

#[test]
fn oversized_graph_data_is_rejected_during_decode() {
    let node = r#"{"label":"n"}"#;
    let nodes = std::iter::repeat_n(node, 65_537)
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(r#"{{"nodes":[{nodes}],"edges":[]}}"#);

    let error = serde_json::from_str::<Graph>(&json).unwrap_err();
    assert!(error.to_string().contains("graph nodes"));
    assert!(error.to_string().contains("limit"));

    let tags = std::iter::repeat_n(r#""t""#, 1_025)
        .collect::<Vec<_>>()
        .join(",");
    let json =
        format!(r#"{{"nodes":[{node},{node}],"edges":[{{"from":0,"to":1,"tags":[{tags}]}}]}}"#);

    let error = serde_json::from_str::<Graph>(&json).unwrap_err();
    assert!(error.to_string().contains("edge tags"));
}

#[test]
fn a_defect_rebinds_to_its_graph() {
    let graph = graph();
    let edge = graph.edge_ids().next().unwrap();
    let defect = Defect {
        at: (3, 4),
        fault: Fault::Crossing,
        edges: (edge, edge),
    };
    let json = serde_json::to_string(&defect).unwrap();
    assert_eq!(json, r#"{"at":[3,4],"fault":"crossing","edges":[0,0]}"#);

    let mut deserializer = serde_json::Deserializer::from_str(&json);
    let decoded = Defect::deserialize_with(&graph, &mut deserializer).unwrap();
    assert_eq!(decoded, defect);
}

#[test]
fn a_drawing_has_a_stable_indexed_wire_shape() {
    let graph = graph();
    let node = graph.node_ids().next().unwrap();
    let edge = graph.edge_ids().next().unwrap();
    let mut drawing = Drawing::new(4, 4).unwrap();
    drawing.boxed(node, 0, Rect::new(0, 0, 2, 2)).unwrap();
    drawing.route(edge, [(1, 1), (3, 1)]).unwrap();

    assert_eq!(
        serde_json::to_string(&drawing).unwrap(),
        r#"{"width":4,"height":4,"boxes":[{"node":0,"column":0,"x":0,"y":0,"w":2,"h":2}],"routes":[{"edge":0,"points":[[1,1],[3,1]]}]}"#
    );
}
