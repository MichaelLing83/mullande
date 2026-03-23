#!/usr/bin/env python3
"""Tests for Memory API atomic operations"""

from mullande.workspace import Memory, WorkspaceManager
import pytest


def test_basic_read_write():
    """Test basic read and write operations"""
    # Reset to clean state for testing
    workspace = WorkspaceManager()
    m = Memory(workspace)

    # Test initial state
    assert ".gitignore" in m.list_files()

    # Test atomic write of multiple files
    success = m.write_atomic(
        {"note1.txt": "First note", "subdir/note2.md": "# Second note"},
        "Add test notes",
    )

    assert success is True
    files = m.list_files()
    assert "note1.txt" in files
    assert "subdir/note2.md" in files

    # Test reading
    assert m.read("note1.txt") == "First note"
    assert m.exists("note1.txt") is True
    assert m.exists("nonexistent.txt") is False


def test_atomic_rollback_on_failure():
    """Test that all changes are rolled back on failure"""
    workspace = WorkspaceManager()
    m = Memory(workspace)

    # Count files before
    files_before = set(m.list_files())

    # First create a file 'foo'
    success = m.write_one("foo", "bar", "Add foo file")
    assert success is True

    # Try to write to foo/baz which should fail
    success = m.write_atomic(
        {
            "should_not_exist.txt": "This should not be here after rollback",
            "foo/baz": "Will fail because foo is a file not directory",
        },
        "This should fail",
    )

    # Write should fail
    assert success is False

    # Check that should_not_exist.txt was rolled back
    files_after = set(m.list_files())
    assert "should_not_exist.txt" not in files_after


def test_single_write():
    """Test single file write"""
    m = Memory()
    success = m.write_one("single.txt", "Single file content", "Add single file")
    assert success is True
    assert m.read("single.txt") == "Single file content"
    assert "single.txt" in m.list_files()
