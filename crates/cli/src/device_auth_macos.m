#import <Foundation/Foundation.h>
#import <LocalAuthentication/LocalAuthentication.h>
#import <Security/Security.h>
#import <dispatch/dispatch.h>
#include <string.h>

enum {
    FRESNICA_MAC_AUTH_AUTHENTICATED = 0,
    FRESNICA_MAC_AUTH_CANCELLED = 1,
    FRESNICA_MAC_AUTH_UNAVAILABLE = 2,
    FRESNICA_MAC_AUTH_FAILED = 3,
};

int fresnica_macos_local_auth_available(void) {
    @autoreleasepool {
        LAContext *context = [[LAContext alloc] init];
        NSError *error = nil;
        return [context canEvaluatePolicy:LAPolicyDeviceOwnerAuthentication error:&error] ? 1 : 0;
    }
}

int fresnica_macos_authenticate(const char *reason, long *error_code) {
    @autoreleasepool {
        if (error_code != NULL) {
            *error_code = 0;
        }

        LAContext *context = [[LAContext alloc] init];
        NSError *preflight_error = nil;
        if (![context canEvaluatePolicy:LAPolicyDeviceOwnerAuthentication error:&preflight_error]) {
            if (error_code != NULL && preflight_error != nil) {
                *error_code = (long)preflight_error.code;
            }
            return FRESNICA_MAC_AUTH_UNAVAILABLE;
        }

        NSString *localized_reason = reason == NULL ? nil : [NSString stringWithUTF8String:reason];
        if (localized_reason.length == 0) {
            return FRESNICA_MAC_AUTH_FAILED;
        }

        dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);
        __block BOOL authenticated = NO;
        __block NSInteger callback_error_code = 0;
        [context evaluatePolicy:LAPolicyDeviceOwnerAuthentication
                localizedReason:localized_reason
                          reply:^(BOOL success, NSError *error) {
                              authenticated = success;
                              callback_error_code = error == nil ? 0 : error.code;
                              dispatch_semaphore_signal(semaphore);
                          }];

        if (dispatch_semaphore_wait(semaphore, dispatch_time(DISPATCH_TIME_NOW, 60 * NSEC_PER_SEC)) != 0) {
            return FRESNICA_MAC_AUTH_FAILED;
        }

        if (error_code != NULL) {
            *error_code = (long)callback_error_code;
        }
        if (authenticated) {
            return FRESNICA_MAC_AUTH_AUTHENTICATED;
        }
        if (callback_error_code == LAErrorUserCancel || callback_error_code == LAErrorUserFallback) {
            return FRESNICA_MAC_AUTH_CANCELLED;
        }
        return FRESNICA_MAC_AUTH_FAILED;
    }
}

int fresnica_macos_refresh_keychain_item_access(
    void *keychain_ref,
    const char *source_service,
    const char *target_service,
    const char *account,
    const char *label
) {
    @autoreleasepool {
        if (keychain_ref == NULL || source_service == NULL || target_service == NULL ||
            account == NULL || label == NULL) {
            return errSecParam;
        }

        SecKeychainItemRef item = NULL;
        OSStatus status = SecKeychainFindGenericPassword(
            (SecKeychainRef)keychain_ref,
            (UInt32)strlen(source_service), source_service,
            (UInt32)strlen(account), account,
            NULL, NULL,
            &item
        );
        if (status != errSecSuccess) {
            return status;
        }

        SecAccessRef access = NULL;
        status = SecKeychainItemCopyAccess(item, &access);
        if (status != errSecSuccess) {
            CFRelease(item);
            return status;
        }

        CFArrayRef acl_list = SecAccessCopyMatchingACLList(access, kSecACLAuthorizationDecrypt);
        if (acl_list == NULL || CFArrayGetCount(acl_list) == 0) {
            if (acl_list != NULL) CFRelease(acl_list);
            CFRelease(access);
            CFRelease(item);
            return errSecInvalidACL;
        }

        SecTrustedApplicationRef current_app = NULL;
        status = SecTrustedApplicationCreateFromPath(NULL, &current_app);
        if (status != errSecSuccess) {
            CFRelease(acl_list);
            CFRelease(access);
            CFRelease(item);
            return status;
        }

        CFDataRef current_data = NULL;
        status = SecTrustedApplicationCopyData(current_app, &current_data);
        if (status != errSecSuccess) {
            CFRelease(current_app);
            CFRelease(acl_list);
            CFRelease(access);
            CFRelease(item);
            return status;
        }

        BOOL access_changed = NO;
        for (CFIndex i = 0; i < CFArrayGetCount(acl_list); ++i) {
            SecACLRef acl = (SecACLRef)CFArrayGetValueAtIndex(acl_list, i);
            CFArrayRef applications = NULL;
            CFStringRef description = NULL;
            SecKeychainPromptSelector prompt_selector = 0;
            status = SecACLCopyContents(acl, &applications, &description, &prompt_selector);
            if (status != errSecSuccess) {
                break;
            }

            // A NULL application list already trusts every application. Preserve
            // that existing policy instead of broadening or narrowing it here.
            if (applications != NULL) {
                BOOL already_trusted = NO;
                for (CFIndex j = 0; j < CFArrayGetCount(applications); ++j) {
                    SecTrustedApplicationRef app =
                        (SecTrustedApplicationRef)CFArrayGetValueAtIndex(applications, j);
                    CFDataRef app_data = NULL;
                    OSStatus data_status = SecTrustedApplicationCopyData(app, &app_data);
                    if (data_status != errSecSuccess) {
                        status = data_status;
                        break;
                    }
                    already_trusted = CFEqual(current_data, app_data);
                    CFRelease(app_data);
                    if (already_trusted) {
                        break;
                    }
                }

                if (status == errSecSuccess && !already_trusted) {
                    CFMutableArrayRef updated = CFArrayCreateMutableCopy(
                        kCFAllocatorDefault, 0, applications
                    );
                    if (updated == NULL) {
                        status = errSecAllocate;
                    } else {
                        CFArrayAppendValue(updated, current_app);
                        CFStringRef prompt_description = description != NULL
                            ? description
                            : (__bridge CFStringRef)[NSString stringWithUTF8String:label];
                        status = SecACLSetContents(
                            acl, updated, prompt_description, prompt_selector
                        );
                        CFRelease(updated);
                        if (status == errSecSuccess) {
                            access_changed = YES;
                        }
                    }
                }
            }

            if (applications != NULL) CFRelease(applications);
            if (description != NULL) CFRelease(description);
            if (status != errSecSuccess) {
                break;
            }
        }

        if (status == errSecSuccess && access_changed) {
            // The copied access keeps the original owner and safe ACLs intact.
            // Only the restricted/decrypt trusted-app list changed above, so
            // Keychain should require a single ChangeACL owner authorization.
            status = SecKeychainItemSetAccess(item, access);
        }

        CFRelease(current_data);
        CFRelease(current_app);
        CFRelease(acl_list);
        CFRelease(access);

        if (status == errSecSuccess) {
            NSString *source = [NSString stringWithUTF8String:source_service];
            NSString *target = [NSString stringWithUTF8String:target_service];
            NSString *account_name = [NSString stringWithUTF8String:account];
            NSString *item_label = [NSString stringWithUTF8String:label];
            if (source == nil || target == nil || account_name == nil || item_label == nil) {
                status = errSecParam;
            } else {
                NSDictionary *query = @{
                    (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
                    (__bridge id)kSecAttrService: source,
                    (__bridge id)kSecAttrAccount: account_name,
                    (__bridge id)kSecMatchSearchList: @[(__bridge id)(SecKeychainRef)keychain_ref],
                };
                NSMutableDictionary *updates = [@{
                    (__bridge id)kSecAttrLabel: item_label,
                } mutableCopy];
                if (![source isEqualToString:target]) {
                    updates[(__bridge id)kSecAttrService] = target;
                }
                status = SecItemUpdate(
                    (__bridge CFDictionaryRef)query,
                    (__bridge CFDictionaryRef)updates
                );
            }
        }

        CFRelease(item);
        return status;
    }
}
