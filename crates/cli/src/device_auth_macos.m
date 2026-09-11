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

int fresnica_macos_keychain_item_trusts_current_application(
    void *keychain_ref,
    const char *service,
    const char *account,
    int *trusted
) {
    @autoreleasepool {
        if (keychain_ref == NULL || service == NULL || account == NULL || trusted == NULL) {
            return errSecParam;
        }
        *trusted = 0;

        SecKeychainItemRef item = NULL;
        OSStatus status = SecKeychainFindGenericPassword(
            (SecKeychainRef)keychain_ref,
            (UInt32)strlen(service), service,
            (UInt32)strlen(account), account,
            NULL, NULL,
            &item
        );
        if (status != errSecSuccess) {
            return status;
        }

        SecAccessRef access = NULL;
        status = SecKeychainItemCopyAccess(item, &access);
        CFRelease(item);
        if (status != errSecSuccess) {
            return status;
        }

        CFArrayRef acl_list = SecAccessCopyMatchingACLList(access, kSecACLAuthorizationDecrypt);
        if (acl_list == NULL) {
            CFRelease(access);
            return errSecInvalidACL;
        }

        SecTrustedApplicationRef current_app = NULL;
        status = SecTrustedApplicationCreateFromPath(NULL, &current_app);
        if (status != errSecSuccess) {
            CFRelease(acl_list);
            CFRelease(access);
            return status;
        }

        CFDataRef current_data = NULL;
        status = SecTrustedApplicationCopyData(current_app, &current_data);
        if (status != errSecSuccess) {
            CFRelease(current_app);
            CFRelease(acl_list);
            CFRelease(access);
            return status;
        }

        for (CFIndex i = 0; i < CFArrayGetCount(acl_list) && !*trusted; ++i) {
            SecACLRef acl = (SecACLRef)CFArrayGetValueAtIndex(acl_list, i);
            CFArrayRef applications = NULL;
            CFStringRef description = NULL;
            SecKeychainPromptSelector prompt_selector = 0;
            status = SecACLCopyContents(acl, &applications, &description, &prompt_selector);
            if (status != errSecSuccess) {
                break;
            }

            if (applications == NULL) {
                *trusted = 1;
            } else {
                for (CFIndex j = 0; j < CFArrayGetCount(applications); ++j) {
                    SecTrustedApplicationRef app =
                        (SecTrustedApplicationRef)CFArrayGetValueAtIndex(applications, j);
                    CFDataRef app_data = NULL;
                    OSStatus data_status = SecTrustedApplicationCopyData(app, &app_data);
                    if (data_status == errSecSuccess) {
                        if (CFEqual(current_data, app_data)) {
                            *trusted = 1;
                        }
                        CFRelease(app_data);
                    } else {
                        status = data_status;
                        break;
                    }
                }
            }

            if (applications != NULL) {
                CFRelease(applications);
            }
            if (description != NULL) {
                CFRelease(description);
            }
            if (status != errSecSuccess) {
                break;
            }
        }

        CFRelease(current_data);
        CFRelease(current_app);
        CFRelease(acl_list);
        CFRelease(access);
        return status;
    }
}
