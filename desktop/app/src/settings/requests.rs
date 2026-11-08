// SPDX-License-Identifier: AGPL-3.0-or-later

use serde_json::Value;

use crate::api::{self, Api};

pub async fn request(api: &Api, operation: &str, variables: Value) -> Result<Value, api::Error> {
    macro_rules! send {
        ($query:ident) => {
            api.request_data::<api::$query, Value>(
                serde_json::from_value(variables).map_err(|e| api::Error::message(e.to_string()))?,
            )
            .await
        };
    }
    match operation {
        "Libraries" => send!(SettingsLibraries),
        "Settings" => send!(SettingsSettings),
        "Scan" => send!(SettingsScan),
        "AddLibrary" => send!(SettingsAddLibrary),
        "UpdateLibrary" => send!(SettingsUpdateLibrary),
        "RemoveLibrary" => send!(SettingsRemoveLibrary),
        "Folders" => send!(SettingsFolders),
        "Passkeys" => send!(SettingsPasskeys),
        "ChangePassword" => send!(SettingsChangePassword),
        "MyEmail" => send!(SettingsMyEmail),
        "SetEmail" => send!(SettingsSetEmail),
        "SendTestEmail" => send!(SettingsSendTestEmail),
        "ReplaceConfig" => send!(SettingsReplaceConfig),
        "SkippedFiles" => send!(SettingsSkippedFiles),
        "SaveSection" => send!(SettingsSaveSection),
        "SettingsEngine" => send!(SettingsSettingsEngine),
        "AddSource" => send!(SettingsAddSource),
        "UpdateSource" => send!(SettingsUpdateSource),
        "RemoveSource" => send!(SettingsRemoveSource),
        "DetectSource" => send!(SettingsDetectSource),
        "AddProfile" => send!(SettingsAddProfile),
        "UpdateProfile" => send!(SettingsUpdateProfile),
        "RemoveProfile" => send!(SettingsRemoveProfile),
        "RenameSuggestions" => send!(SettingsRenameSuggestions),
        "FileHistory" => send!(SettingsFileHistory),
        "RefreshRenameSuggestions" => send!(SettingsRefreshRenameSuggestions),
        "ApplyRenames" => send!(SettingsApplyRenames),
        "DismissRenames" => send!(SettingsDismissRenames),
        "UndoFileChanges" => send!(SettingsUndoFileChanges),
        "ClipStorage" => send!(SettingsClipStorage),
        "DropClipRenders" => send!(SettingsDropClipRenders),
        "UnpublishClip" => send!(SettingsUnpublishClip),
        "AdminDeleteClip" => send!(SettingsAdminDeleteClip),
        "AppPasswords" => send!(SettingsAppPasswords),
        "CreateAppPassword" => send!(SettingsCreateAppPassword),
        "DeleteAppPassword" => send!(SettingsDeleteAppPassword),
        "Users" => send!(SettingsUsers),
        "PermissionDefaults" => send!(SettingsPermissionDefaults),
        "SetPermissionDefaults" => send!(SettingsSetPermissionDefaults),
        "UpdateUser" => send!(SettingsUpdateUser),
        "DeleteUser" => send!(SettingsDeleteUser),
        "CreateUser" => send!(SettingsCreateUser),
        "Invites" => send!(SettingsInvites),
        "CreateInvite" => send!(SettingsCreateInvite),
        "RevokeInvite" => send!(SettingsRevokeInvite),
        "ColorSchemes" => send!(SettingsColorSchemes),
        "AppearanceSettings" => send!(SettingsAppearanceSettings),
        "ServerAppearance" => send!(SettingsServerAppearance),
        "SetAppearance" => send!(SettingsSetAppearance),
        "SetServerAppearance" => send!(SettingsSetServerAppearance),
        "SaveScheme" => send!(SettingsSaveScheme),
        "ForkScheme" => send!(SettingsForkScheme),
        "ImportScheme" => send!(SettingsImportScheme),
        "DeleteScheme" => send!(SettingsDeleteScheme),
        "PublishScheme" => send!(SettingsPublishScheme),
        _ => Err(api::Error::message("Unknown settings operation.")),
    }
}
