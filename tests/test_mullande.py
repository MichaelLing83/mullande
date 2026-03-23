"""
Tests for mullande
"""
from mullande import __version__
from mullande.agent import AgentSystem


def test_version():
    assert __version__ == "0.1.0"


def test_agent_process():
    agent = AgentSystem()
    response = agent.process("Hello world")
    assert "Hello world" in response.content
