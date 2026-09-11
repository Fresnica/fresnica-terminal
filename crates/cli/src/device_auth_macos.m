#import <Foundation/Foundation.h>
#import <LocalAuthentication/LocalAuthentication.h>
#import <Security/Security.h>
#import <dispatch/dispatch.h>

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

        NSString *source = [NSString stringWithUTF8String:source_service];
        NSString *target = [NSString stringWithUTF8String:target_service];
        NSString *account_name = [NSString stringWithUTF8String:account];
        NSString *item_label = [NSString stringWithUTF8String:label];
        if (source == nil || target == nil || account_name == nil || item_label == nil) {
            return errSecParam;
        }

        SecAccessRef access = NULL;
        OSStatus status = SecAccessCreate((__bridge CFStringRef)item_label, NULL, &access);
        if (status != errSecSuccess) {
            return status;
        }

        NSDictionary *query = @{
            (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
            (__bridge id)kSecAttrService: source,
            (__bridge id)kSecAttrAccount: account_name,
            (__bridge id)kSecMatchSearchList: @[(__bridge id)(SecKeychainRef)keychain_ref],
        };
        NSDictionary *updates = @{
            (__bridge id)kSecAttrAccess: (__bridge id)access,
            (__bridge id)kSecAttrService: target,
            (__bridge id)kSecAttrLabel: item_label,
            (__bridge id)kSecAttrDescription: @"Fresnica Device Unlock key",
        };
        status = SecItemUpdate(
            (__bridge CFDictionaryRef)query,
            (__bridge CFDictionaryRef)updates
        );
        CFRelease(access);
        return status;
    }
}
