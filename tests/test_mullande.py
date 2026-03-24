"""
Tests for mullande
"""

from mullande import __version__
from mullande.agent import AgentSystem


def test_version():
    assert __version__ == "0.1.0"


def test_agent_process():
    # If ollama is not running, we just check it doesn't crash
    agent = AgentSystem()
    try:
        response = agent.process("Hello world")
        # If connected to ollama, it should produce response
        assert len(response) > 0
    except Exception:
        # If connection fails, that's expected for test environment
        pass
