"""
Core Agent system implementation for mullande
"""

from typing import Optional, List, Dict, Any
from pydantic import BaseModel

from mullande.config import get_config, Config, ModelConfig


class AgentResponse(BaseModel):
    """Response from the Agent system"""

    content: str
    metadata: Dict[str, Any] = {}


class AgentSystem:
    """Main Agent system for large model interactions"""

    def __init__(self, model: Optional[str] = None):
        """Initialize the Agent system with optional model specification"""
        self.config = get_config()
        self.requested_model = model
        self.model_config = self.config.get_model_config(model)
        self.conversation_history: List[str] = []

    @property
    def effective_model_id(self) -> str:
        """Get effective model ID"""
        if self.requested_model and self.model_config.model_id:
            return self.model_config.model_id
        if self.requested_model:
            return self.requested_model
        if self.model_config.model_id:
            return self.model_config.model_id
        return "unknown"

    def get_api_key(self) -> Optional[str]:
        """Get API key from environment as configured"""
        return self.config.get_api_key(self.requested_model)

    def get_context_window(self) -> int:
        """Get effective context window size"""
        return self.config.get_context_window(self.requested_model)

    def process(self, input_text: str) -> str:
        """Process input text through the Agent system and return response"""
        self.conversation_history.append(input_text)

        # Get model configuration
        provider = self.model_config.provider
        model_id = self.effective_model_id
        context_window = self.get_context_window()
        api_key = self.get_api_key()

        # In future implementation, this will actually call the LLM API
        # For now, return a response with configuration info
        response = f"Processed by {provider} model {model_id}:\n{input_text}\n\n"
        response += f"Configuration details:\n"
        response += f"- Provider: {provider}\n"
        response += f"- Model: {model_id}\n"
        response += f"- Context window: {context_window}\n"

        if api_key:
            response += f"- API key loaded from environment: ✓\n"
        else:
            response += f"- API key: not loaded from environment\n"

        if self.model_config.base_url:
            response += f"- Base URL: {self.model_config.base_url}\n"

        return response

    def start_chat(self) -> None:
        """Start an interactive chat session"""
        # Placeholder for interactive chat implementation
        import readline

        try:
            while True:
                prompt = input("You > ")
                response = self.process(prompt)
                print(f"Agent > {response.content}")
        except KeyboardInterrupt:
            print("\nExiting chat...")
