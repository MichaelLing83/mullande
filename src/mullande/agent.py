"""
Core Agent system implementation for mullande
"""

import json
import httpx
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

        if provider == "ollama":
            return self._call_ollama(input_text, model_id, context_window)
        elif provider in ["volcengine", "copilot"]:
            # Will implement these providers later
            response = f"Provider {provider} not implemented yet.\n"
            response += f"Configuration:\n"
            response += f"- Provider: {provider}\n"
            response += f"- Model: {model_id}\n"
            response += f"- Context window: {context_window}\n"
            if api_key:
                response += "- API key loaded from environment: ✓\n"
            return response
        else:
            return f"Unknown provider: {provider}"

    def _call_ollama(self, prompt: str, model: str, context_window: int) -> str:
        """Call ollama API with the given prompt"""
        base_url = self.model_config.base_url or "http://localhost:11434"
        api_url = f"{base_url.rstrip('/')}/api/generate"

        payload = {
            "model": model,
            "prompt": prompt,
            "stream": False,
            "options": {"num_ctx": context_window},
        }

        headers = {}
        api_key = self.get_api_key()
        if api_key:
            headers["Authorization"] = f"Bearer {api_key}"

        try:
            response = httpx.post(api_url, json=payload, headers=headers, timeout=None)
            response.raise_for_status()
            data = response.json()
            return data.get("response", "No response from model")
        except (httpx.RequestError, httpx.HTTPStatusError) as e:
            return f"Error connecting to ollama at {api_url}: {e}\nPlease ensure ollama is running and the model '{model}' is pulled.\nHint: Run 'ollama pull {model}' to download the model first."

    def start_chat(self) -> None:
        """Start an interactive chat session"""
        # Placeholder for interactive chat implementation
        import readline

        try:
            while True:
                prompt = input("You > ")
                response = self.process(prompt)
                print(f"Agent > {response}")
        except KeyboardInterrupt:
            print("\nExiting chat...")
