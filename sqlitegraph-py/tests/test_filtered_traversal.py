"""Tests for kind-filtered graph traversal (bfs, shortest_path, k_hop)."""

import sqlitegraph


def _g():
    return sqlitegraph.Graph.open_in_memory()


def _typed_chain():
    g = _g()
    a = g.add_node(kind="N", name="a")
    b = g.add_node(kind="N", name="b")
    c = g.add_node(kind="N", name="c")
    d = g.add_node(kind="N", name="d")
    g.add_edge(a, b, "CALL")
    g.add_edge(b, c, "CALL")
    g.add_edge(a, d, "IMPORTS")
    g.add_edge(d, c, "IMPORTS")
    return g, a, b, c, d


def test_bfs_filter_allows_only_specified_edge_type():
    g, a, b, c, d = _typed_chain()
    reached = g.bfs(a, depth=10, edge_types=["CALL"])
    assert a in reached
    assert b in reached
    assert c in reached
    assert d not in reached


def test_bfs_filter_with_multiple_edge_types_unions_neighbors():
    g, a, b, c, d = _typed_chain()
    reached = g.bfs(a, depth=10, edge_types=["CALL", "IMPORTS"])
    assert set(reached) >= {a, b, c, d}


def test_bfs_filter_empty_returns_only_start():
    g, a, _b, _c, _d = _typed_chain()
    reached = g.bfs(a, depth=10, edge_types=[])
    assert reached == []


def test_bfs_filter_incoming_direction():
    g = _g()
    a = g.add_node(kind="N", name="a")
    b = g.add_node(kind="N", name="b")
    c = g.add_node(kind="N", name="c")
    g.add_edge(a, c, "CALL")
    g.add_edge(b, c, "IMPORTS")
    reached = g.bfs(c, depth=10, edge_types=["CALL"], direction="incoming")
    assert a in reached
    assert b not in reached


def test_bfs_without_filter_still_works():
    """Backward compatibility: bfs() with no kwargs returns unfiltered result."""
    g, a, b, c, d = _typed_chain()
    reached = g.bfs(a, depth=10)
    assert set(reached) >= {a, b, c, d}


def test_shortest_path_filter_picks_path_through_allowed_kind():
    g, a, b, c, d = _typed_chain()
    path = g.shortest_path(a, c, edge_types=["CALL"])
    assert path == [a, b, c]
    assert d not in path

    path_imports = g.shortest_path(a, c, edge_types=["IMPORTS"])
    assert path_imports == [a, d, c]
    assert b not in path_imports


def test_shortest_path_filter_returns_none_when_kind_excludes_all_paths():
    g, a, _b, c, _d = _typed_chain()
    path = g.shortest_path(a, c, edge_types=["UNKNOWN_KIND"])
    assert path is None


def test_shortest_path_filter_empty_returns_none():
    g, a, _b, c, _d = _typed_chain()
    path = g.shortest_path(a, c, edge_types=[])
    assert path is None


def test_shortest_path_without_filter_still_works():
    """Backward compatibility: shortest_path() with no kwargs returns unfiltered result."""
    g, a, _b, c, _d = _typed_chain()
    path = g.shortest_path(a, c)
    assert path is not None
    assert path[0] == a and path[-1] == c


def test_k_hop_filter_excludes_neighbors_of_other_kinds():
    g = _g()
    a = g.add_node(kind="N", name="a")
    b = g.add_node(kind="N", name="b")
    c = g.add_node(kind="N", name="c")
    g.add_edge(a, b, "CALL")
    g.add_edge(a, c, "IMPORTS")
    hops = g.k_hop(a, depth=1, edge_types=["CALL"])
    assert b in hops
    assert c not in hops


def test_k_hop_without_filter_still_works():
    """Backward compatibility: k_hop() with no edge_types returns unfiltered."""
    g = _g()
    a = g.add_node(kind="N", name="a")
    b = g.add_node(kind="N", name="b")
    g.add_edge(a, b, "CALL")
    hops = g.k_hop(a, depth=1)
    assert b in hops
