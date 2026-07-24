use warpui::AppContext;

use super::{CloudObject, GenericStringObjectFormat, JsonObjectType, ObjectType};
use crate::i18n::{tr, Message};
use crate::server::cloud_objects::update_manager::{
    InitiatedBy, ObjectOperation, OperationSuccessType,
};

pub struct CloudObjectToastMessage;

impl CloudObjectToastMessage {
    pub fn toast_message(
        object: &dyn CloudObject,
        operation: &ObjectOperation,
        success_type: &OperationSuccessType,
        app: &AppContext,
    ) -> Option<String> {
        let object_name = object.model_type_name().to_owned();
        let object_name_lowercase = object_name.to_ascii_lowercase();

        match (object.object_type(), operation, success_type) {
            // We should only show toasts for creates initiated by the user, not by the system
            (
                _,
                ObjectOperation::Create {
                    initiated_by: InitiatedBy::User,
                },
                OperationSuccessType::Success,
            ) => {
                let containing_object_name = object.containing_object_name(app);
                Some(
                    tr(app, Message::DriveToastSavedTo)
                        .replacen("{}", &object_name, 1)
                        .replacen("{}", &containing_object_name, 1),
                )
            }
            // notebooks intentionally do not have an update message, as they are updated
            // as the user types and so toasts would be VERY noisy
            (ObjectType::Notebook, ObjectOperation::Update, OperationSuccessType::Success) => None,
            (_, ObjectOperation::Update, OperationSuccessType::Success) => {
                Some(tr(app, Message::DriveToastUpdated).replace("{}", &object_name))
            }
            (_, ObjectOperation::MoveToFolder, OperationSuccessType::Success)
            | (_, ObjectOperation::MoveToDrive, OperationSuccessType::Success) => {
                let containing_object_name = object.containing_object_name(app);
                Some(
                    tr(app, Message::DriveToastMovedTo)
                        .replacen("{}", &object_name, 1)
                        .replacen("{}", &containing_object_name, 1),
                )
            }
            (_, ObjectOperation::Trash, OperationSuccessType::Success) => {
                Some(tr(app, Message::DriveToastTrashed).replace("{}", &object_name))
            }
            (_, ObjectOperation::Untrash, OperationSuccessType::Success) => {
                Some(tr(app, Message::DriveToastRestored).replace("{}", &object_name))
            }
            (_, ObjectOperation::Leave, OperationSuccessType::Success) => {
                Some(tr(app, Message::DriveToastLeft).replace("{}", &object_name))
            }
            (
                _,
                ObjectOperation::Create {
                    initiated_by: InitiatedBy::User,
                },
                OperationSuccessType::Failure,
            ) => {
                Some(tr(app, Message::DriveToastFailedCreate).replace("{}", &object_name_lowercase))
            }
            (
                _,
                ObjectOperation::Create {
                    initiated_by: InitiatedBy::User,
                },
                OperationSuccessType::Denied(message),
            ) => Some(message.to_string()),
            (_, ObjectOperation::Update, OperationSuccessType::Failure) => {
                Some(tr(app, Message::DriveToastFailedUpdate).replace("{}", &object_name_lowercase))
            }
            (_, ObjectOperation::MoveToFolder, OperationSuccessType::Failure)
            | (_, ObjectOperation::MoveToDrive, OperationSuccessType::Failure) => {
                Some(tr(app, Message::DriveToastFailedMove).replace("{}", &object_name_lowercase))
            }
            (_, ObjectOperation::Trash, OperationSuccessType::Failure) => {
                Some(tr(app, Message::DriveToastFailedTrash).replace("{}", &object_name_lowercase))
            }
            (_, ObjectOperation::Untrash, OperationSuccessType::Failure) => Some(
                tr(app, Message::DriveToastFailedRestore).replace("{}", &object_name_lowercase),
            ),
            // We should only show deletion failure toasts for user-initiated deletions.
            (
                _,
                ObjectOperation::Delete {
                    initiated_by: InitiatedBy::User,
                },
                OperationSuccessType::Failure,
            ) => {
                Some(tr(app, Message::DriveToastFailedDelete).replace("{}", &object_name_lowercase))
            }
            (_, ObjectOperation::Leave, OperationSuccessType::Failure) => {
                Some(tr(app, Message::DriveToastFailedLeave).replace("{}", &object_name))
            }
            (ObjectType::Workflow, ObjectOperation::Update, OperationSuccessType::Rejection) => {
                Some(tr(app, Message::DriveToastWorkflowConflict).to_string())
            }
            (
                ObjectType::GenericStringObject(GenericStringObjectFormat::Json(
                    JsonObjectType::EnvVarCollection,
                )),
                ObjectOperation::Update,
                OperationSuccessType::Rejection,
            ) => Some(tr(app, Message::DriveToastEnvVarsConflict).to_string()),
            (
                ObjectType::GenericStringObject(GenericStringObjectFormat::Json(
                    JsonObjectType::AIFact,
                )),
                ObjectOperation::Update,
                OperationSuccessType::Rejection,
            ) => Some(tr(app, Message::DriveToastRuleConflict).to_string()),
            (_, ObjectOperation::TakeEditAccess, OperationSuccessType::Failure) => Some(
                tr(app, Message::DriveToastFailedStartEditing)
                    .replace("{}", &object_name_lowercase),
            ),
            (_, ObjectOperation::UpdatePermissions, OperationSuccessType::Success) => Some(
                tr(app, Message::DriveToastUpdatedPermissions)
                    .replace("{}", &object_name_lowercase),
            ),
            (_, ObjectOperation::UpdatePermissions, OperationSuccessType::Failure) => Some(
                tr(app, Message::DriveToastFailedUpdatePermissions)
                    .replace("{}", &object_name_lowercase),
            ),
            _ => None,
        }
    }

    pub fn toast_deletion_confirm_message(
        num_objects: i32,
        operation: &ObjectOperation,
        success_type: &OperationSuccessType,
    ) -> Option<String> {
        use crate::i18n::tr_cached;
        let count_objects_message = match num_objects {
            1 => tr_cached(Message::DriveToastOneObject).to_string(),
            n => tr_cached(Message::DriveToastNObjects).replace("{}", &n.to_string()),
        };
        match (operation, success_type) {
            // We should only show deletion failure toasts for user-initiated deletions.
            (
                ObjectOperation::Delete {
                    initiated_by: InitiatedBy::User,
                },
                OperationSuccessType::Success,
            ) => Some(
                tr_cached(Message::DriveToastDeletedForever).replace("{}", &count_objects_message),
            ),
            (ObjectOperation::EmptyTrash, OperationSuccessType::Success) => Some(
                tr_cached(Message::DriveToastTrashEmptied).replace("{}", &count_objects_message),
            ),
            (ObjectOperation::EmptyTrash, OperationSuccessType::Failure) => {
                Some(tr_cached(Message::DriveToastFailedEmptyTrash).to_string())
            }
            (ObjectOperation::EmptyTrash, OperationSuccessType::Rejection) => {
                Some(tr_cached(Message::DriveToastNoObjectsInTrash).to_string())
            }
            _ => None,
        }
    }
}
