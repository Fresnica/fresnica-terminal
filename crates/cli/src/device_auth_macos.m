#import <Foundation/Foundation.h>
#import <LocalAuthentication/LocalAuthentication.h>
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
