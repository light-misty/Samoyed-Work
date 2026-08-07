use serde::{Deserialize, Serialize};

/// 快照记录（session_snapshots 表）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRecord {
    pub id: String,
    pub session_id: String,
    /// 关联的 user 消息 ID；NULL 表示 redo 基线快照
    pub message_id: Option<String>,
    /// 快照类型：git / files
    pub kind: String,
    /// git SHA 或文件备份目录路径
    pub snapshot_ref: String,
    pub workspace_path: String,
    pub created_at: String,
}

/// 快照简要信息（返回给前端，用于恢复快照节点展示）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotInfo {
    pub message_id: String,
    /// 快照类型：git / files
    pub kind: String,
    pub created_at: String,
}

/// 回退状态信息（返回给前端，用于显示"已回退"横幅）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RevertInfo {
    /// 回退边界消息 ID（该消息及之后被隐藏）
    pub revert_message_id: String,
    /// 被隐藏的消息数量
    pub hidden_count: usize,
    /// 快照类型：git / files
    pub snapshot_kind: String,
}

/// 回退命令返回结果
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RollbackResult {
    /// 回退边界消息 ID
    pub revert_message_id: String,
    /// 被隐藏的消息数量
    pub hidden_count: usize,
    /// 恢复的文件数量
    pub restored_file_count: usize,
    /// 代码是否已回退（目标消息无快照时为 false，仅回退对话）
    pub code_reverted: bool,
    /// 快照类型：git / files（无快照时为 None）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_kind: Option<String>,
}

/// 撤销回退（redo）命令返回结果
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RedoResult {
    /// 恢复显示的消息数量
    pub hidden_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 RollbackResult 序列化为 camelCase
    #[test]
    fn test_rollback_result_serialization() {
        let result = RollbackResult {
            revert_message_id: "msg_2".to_string(),
            hidden_count: 3,
            restored_file_count: 2,
            code_reverted: true,
            snapshot_kind: Some("git".to_string()),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["revertMessageId"], "msg_2");
        assert_eq!(json["hiddenCount"], 3);
        assert_eq!(json["restoredFileCount"], 2);
        assert_eq!(json["codeReverted"], true);
        assert_eq!(json["snapshotKind"], "git");
    }

    /// 测试无快照时 codeReverted=false、snapshotKind=None 被跳过
    #[test]
    fn test_rollback_result_no_snapshot() {
        let result = RollbackResult {
            revert_message_id: "msg_1".to_string(),
            hidden_count: 1,
            restored_file_count: 0,
            code_reverted: false,
            snapshot_kind: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["codeReverted"], false);
        assert!(json.get("snapshotKind").is_none());
    }

    /// 测试 RevertInfo 序列化
    #[test]
    fn test_revert_info_serialization() {
        let info = RevertInfo {
            revert_message_id: "msg_3".to_string(),
            hidden_count: 2,
            snapshot_kind: "files".to_string(),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["revertMessageId"], "msg_3");
        assert_eq!(json["hiddenCount"], 2);
        assert_eq!(json["snapshotKind"], "files");
    }
}
