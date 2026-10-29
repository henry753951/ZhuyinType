#pragma once
#include "BrokerClient.h"
#include <msctf.h>
#include <wrl/client.h>
#include <functional>
namespace zt {
template<class T> using Com = Microsoft::WRL::ComPtr<T>;
// All text/range operations run only under the context's write cookie.
HRESULT ApplyReply(ITfContext* context, TfEditCookie cookie, ITfCompositionSink* sink,
                   Com<ITfComposition>& composition, const Reply& reply);
HRESULT FinishComposition(TfEditCookie cookie, Com<ITfComposition>& composition, bool discard);
bool IsSecure(ITfContext* context, TfEditCookie cookie);
void Caret(ITfContext* context, TfEditCookie cookie, RECT& rect, double& scale);
HRESULT Edit(ITfContext* context, TfClientId client, bool sync, std::function<HRESULT(TfEditCookie)> action);
}
