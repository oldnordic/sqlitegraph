"""Tests for the bulk insert primitives: add_nodes_bulk, add_edges_bulk."""

import pytest
import sqlitegraph


def _g():
    return sqlitegraph.Graph.open_in_memory()


def test_add_nodes_bulk_returns_ids_in_input_order():
    g = _g()
    items = [
        {"kind": "Function", "name": "alpha"},
        {"kind": "Function", "name": "beta"},
        {"kind": "Function", "name": "gamma"},
    ]
    ids = g.add_nodes_bulk(items)
    assert len(ids) == 3
    assert ids[0] < ids[1] < ids[2]

    # Verify they actually round-trip.
    middle = g.get_node(ids[1])
    assert middle["name"] == "beta"


def test_add_nodes_bulk_empty_returns_empty():
    g = _g()
    assert g.add_nodes_bulk([]) == []


def test_add_nodes_bulk_accepts_data_and_file_path():
    g = _g()
    items = [
        {
            "kind": "File",
            "name": "main.rs",
            "file_path": "src/main.rs",
            "data": {"loc": 42, "tags": ["entry"]},
        },
    ]
    ids = g.add_nodes_bulk(items)
    assert len(ids) == 1
    node = g.get_node(ids[0])
    assert node["kind"] == "File"
    assert node["name"] == "main.rs"
    assert node["data"]["loc"] == 42
    assert node["data"]["tags"] == ["entry"]


def test_add_nodes_bulk_missing_kind_raises():
    g = _g()
    with pytest.raises(Exception):
        g.add_nodes_bulk([{"name": "alpha"}])


def test_add_nodes_bulk_missing_name_raises():
    g = _g()
    with pytest.raises(Exception):
        g.add_nodes_bulk([{"kind": "Function"}])


def test_add_edges_bulk_returns_ids_in_input_order():
    g = _g()
    node_ids = g.add_nodes_bulk(
        [
            {"kind": "N", "name": "a"},
            {"kind": "N", "name": "b"},
            {"kind": "N", "name": "c"},
        ]
    )
    a, b, c = node_ids
    items = [
        {"from_id": a, "to_id": b, "edge_type": "CALL"},
        {"from_id": b, "to_id": c, "edge_type": "CALL"},
    ]
    edge_ids = g.add_edges_bulk(items)
    assert len(edge_ids) == 2
    assert edge_ids[0] < edge_ids[1]


def test_add_edges_bulk_empty_returns_empty():
    g = _g()
    assert g.add_edges_bulk([]) == []


def test_add_edges_bulk_accepts_data():
    g = _g()
    a, b = g.add_nodes_bulk(
        [{"kind": "N", "name": "a"}, {"kind": "N", "name": "b"}]
    )
    edge_ids = g.add_edges_bulk(
        [{"from_id": a, "to_id": b, "edge_type": "CALL", "data": {"line": 17}}]
    )
    edge = g.get_edge(edge_ids[0])
    assert edge["edge_type"] == "CALL"
    assert edge["data"]["line"] == 17


def test_add_edges_bulk_unknown_endpoint_raises():
    g = _g()
    a, _ = g.add_nodes_bulk(
        [{"kind": "N", "name": "a"}, {"kind": "N", "name": "b"}]
    )
    with pytest.raises(Exception):
        g.add_edges_bulk(
            [{"from_id": a, "to_id": 999_999, "edge_type": "CALL"}]
        )


def test_bulk_matches_single_observable_state():
    """A bulk call produces the same observable graph as a per-item loop."""
    g_bulk = _g()
    g_single = _g()

    items = [
        {"kind": "N", "name": f"node_{i}"} for i in range(50)
    ]
    bulk_ids = g_bulk.add_nodes_bulk(items)
    single_ids = [
        g_single.add_node(kind=item["kind"], name=item["name"]) for item in items
    ]
    assert len(bulk_ids) == len(single_ids)

    # Round-trip names match
    for nid_bulk, nid_single in zip(bulk_ids, single_ids):
        assert g_bulk.get_node(nid_bulk)["name"] == g_single.get_node(nid_single)["name"]
