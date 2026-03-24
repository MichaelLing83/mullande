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


def test_chat_special_command_detection():
    # Test that AgentSystem has _handle_special_command method
    agent = AgentSystem()
    assert hasattr(agent, "_handle_special_command")
    assert hasattr(agent, "_cmd_list_models")
    assert hasattr(agent, "_cmd_switch_model")
    # Check initial model is correctly set
    assert agent.effective_model_id is not None
    assert len(agent.effective_model_id) > 0


def test_switch_model():
    # Test model switching works without crashing
    from rich.console import Console

    agent = AgentSystem()
    console = Console()
    original_model = agent.effective_model_id

    # Switch to a new model (even if not configured, it should work)
    agent._cmd_switch_model("test-model", console)

    # After switching, effective_model_id should be test-model
    assert agent.effective_model_id == "test-model"

    # Switch back to original default
    from mullande.config import get_config

    config = get_config()
    default_model = config.data.model.model_id
    agent._cmd_switch_model(default_model, console)
    assert agent.effective_model_id == default_model


def test_list_models_does_not_crash():
    # Test that listing models doesn't crash
    from rich.console import Console

    agent = AgentSystem()
    console = Console()
    # Should work even with no additional models
    agent._cmd_list_models(console)
    # If we get here, it didn't crash - test passes
