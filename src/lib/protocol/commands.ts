export type StreamingBehavior = "steer" | "followUp";

export type HostCommand =
  | "agent_start"
  | "agent_stop"
  | "agent_restart"
  | "prompt"
  | "steer"
  | "follow_up"
  | "abort"
  | "new_session"
  | "switch_session"
  | "list_sessions"
  | "get_state"
  | "get_messages"
  | "set_model"
  | "set_thinking_level"
  | "get_available_models"
  | "get_session_stats"
  | "compact"
  | "get_commands"
  | "ui_respond"
  | "pick_workspace"
  | "save_secret";