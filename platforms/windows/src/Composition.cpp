#include "Composition.h"
#include "Identity.h"
#include <inputscope.h>
#include <shellscalingapi.h>
#include <new>

namespace zt {
namespace {
class EditSession final : public ITfEditSession {
public:
    explicit EditSession(std::function<HRESULT(TfEditCookie)> action) : action_(std::move(action)) { ++moduleObjects; }
    ~EditSession() { --moduleObjects; }
    STDMETHODIMP QueryInterface(REFIID iid, void** out) override {
        if (!out) return E_POINTER;
        *out=nullptr;
        if (iid!=IID_IUnknown && iid!=IID_ITfEditSession) return E_NOINTERFACE;
        *out=static_cast<ITfEditSession*>(this); AddRef(); return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return ++refs_; }
    STDMETHODIMP_(ULONG) Release() override { const auto n=--refs_; if (!n) delete this; return n; }
    STDMETHODIMP DoEditSession(TfEditCookie cookie) override {
        try { return action_(cookie); } catch (...) { return E_FAIL; }
    }
private:
    ULONG refs_=1;
    std::function<HRESULT(TfEditCookie)> action_;
};
bool Compartment(ITfContext* context, REFGUID id) {
    Com<ITfCompartmentMgr> mgr;
    Com<ITfCompartment> compartment;
    if (FAILED(context->QueryInterface(IID_PPV_ARGS(&mgr))) || FAILED(mgr->GetCompartment(id,&compartment))) return false;
    VARIANT v; VariantInit(&v);
    const bool set=SUCCEEDED(compartment->GetValue(&v)) && v.vt==VT_I4 && v.lVal!=0;
    VariantClear(&v);
    return set;
}
HRESULT Selection(ITfContext* context, TfEditCookie cookie, Com<ITfRange>& range) {
    TF_SELECTION selection{};
    ULONG fetched=0;
    const auto hr=context->GetSelection(cookie,TF_DEFAULT_SELECTION,1,&selection,&fetched);
    if (FAILED(hr)) return hr;
    if (!fetched || !selection.range) return E_FAIL;
    range.Attach(selection.range);
    return S_OK;
}
HRESULT MoveCaret(ITfContext* context, TfEditCookie cookie, ITfRange* range, int cursor) {
    Com<ITfRange> caret;
    auto hr=range->Clone(&caret);
    if (FAILED(hr)) return hr;
    if (FAILED(hr=caret->Collapse(cookie,TF_ANCHOR_START))) return hr;
    LONG moved=0;
    if (FAILED(hr=caret->ShiftStart(cookie,cursor,&moved,nullptr))) return hr;
    if (moved!=cursor) return E_FAIL;
    if (FAILED(hr=caret->Collapse(cookie,TF_ANCHOR_START))) return hr;
    TF_SELECTION selection{caret.Get(),{TF_AE_NONE,FALSE}};
    return context->SetSelection(cookie,1,&selection);
}
}
HRESULT Edit(ITfContext* context, TfClientId client, bool sync, std::function<HRESULT(TfEditCookie)> action) {
    Com<ITfEditSession> session;
    session.Attach(new EditSession(std::move(action)));
    HRESULT result=E_FAIL;
    const auto hr=context->RequestEditSession(client,session.Get(),(sync?TF_ES_SYNC:TF_ES_ASYNCDONTCARE)|TF_ES_READWRITE,&result);
    return FAILED(hr)?hr:result;
}
bool IsSecure(ITfContext* context, TfEditCookie cookie) {
    TF_STATUS status{};
    if (FAILED(context->GetStatus(&status)) || (status.dwDynamicFlags & TS_SD_READONLY)) return true;
    if (Compartment(context,GUID_COMPARTMENT_KEYBOARD_DISABLED) || Compartment(context,GUID_COMPARTMENT_EMPTYCONTEXT)) return true;
    GUITHREADINFO gui{sizeof(gui)};
    if (GetGUIThreadInfo(GetCurrentThreadId(),&gui) && gui.hwndFocus) {
        wchar_t klass[64]{};
        GetClassNameW(gui.hwndFocus,klass,64);
        // ES_PASSWORD shares a bit with unrelated window styles: restrict to edit controls.
        if ((_wcsicmp(klass,L"Edit")==0 || _wcsnicmp(klass,L"RichEdit",8)==0) &&
            (GetWindowLongPtrW(gui.hwndFocus,GWL_STYLE)&ES_PASSWORD)) return true;
    }
    Com<ITfReadOnlyProperty> property;
    Com<ITfRange> range;
    if (FAILED(Selection(context,cookie,range))) return true;
    if (FAILED(context->GetAppProperty(GUID_PROP_INPUTSCOPE,&property))) return false;
    VARIANT v; VariantInit(&v);
    bool secure=false;
    if (SUCCEEDED(property->GetValue(cookie,range.Get(),&v)) && v.vt==VT_UNKNOWN && v.punkVal) {
        Com<ITfInputScope> scopes;
        if (SUCCEEDED(v.punkVal->QueryInterface(IID_PPV_ARGS(&scopes)))) {
            InputScope* values=nullptr; UINT count=0;
            if (SUCCEEDED(scopes->GetInputScopes(&values,&count))) {
                for (UINT i=0;i<count;++i) {
                    switch (values[i]) {
                    case IS_PASSWORD: case IS_NUMERIC_PASSWORD: case IS_NUMERIC_PIN:
                    case IS_ALPHANUMERIC_PIN: case IS_ALPHANUMERIC_PIN_SET: case IS_PRIVATE: secure=true; break;
                    default: break;
                    }
                }
                CoTaskMemFree(values);
            }
        }
    }
    VariantClear(&v);
    return secure;
}
void Caret(ITfContext* context, TfEditCookie cookie, RECT& rect, double& scale) {
    Com<ITfContextView> view;
    Com<ITfRange> range;
    HWND window=nullptr;
    scale=1;
    if (FAILED(context->GetActiveView(&view))) return;
    view->GetWnd(&window);
    BOOL clipped=FALSE;
    if (FAILED(Selection(context,cookie,range)) || FAILED(view->GetTextExt(cookie,range.Get(),&rect,&clipped))) {
        GUITHREADINFO gui{sizeof(gui)};
        if (GetGUIThreadInfo(GetCurrentThreadId(),&gui) && gui.hwndCaret) {
            rect=gui.rcCaret; window=gui.hwndCaret;
            MapWindowPoints(gui.hwndCaret,nullptr,reinterpret_cast<POINT*>(&rect),2);
        }
    }
    // Text stores return coordinates in the owner's DPI space, including unaware apps.
    POINT a{rect.left,rect.top},b{rect.right,rect.bottom};
    if (window && LogicalToPhysicalPointForPerMonitorDPI(window,&a) && LogicalToPhysicalPointForPerMonitorDPI(window,&b))
        rect={a.x,a.y,b.x,b.y};
    const auto previous=SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    DEVICE_SCALE_FACTOR factor=SCALE_100_PERCENT;
    GetScaleFactorForMonitor(MonitorFromRect(&rect,MONITOR_DEFAULTTONEAREST),&factor);
    scale=static_cast<double>(factor)/100;
    if (previous) SetThreadDpiAwarenessContext(previous);
}
HRESULT FinishComposition(TfEditCookie cookie, Com<ITfComposition>& composition, bool discard) {
    if (!composition) return S_OK;
    auto owned=composition;
    if (discard) {
        Com<ITfRange> range;
        const auto hr=owned->GetRange(&range);
        if (FAILED(hr)) return hr;
        if (FAILED(range->SetText(cookie,0,L"",0))) return E_FAIL;
    }
    const auto hr=owned->EndComposition(cookie);
    if (SUCCEEDED(hr)) composition.Reset();
    return hr;
}
HRESULT ApplyReply(ITfContext* context, TfEditCookie cookie, ITfCompositionSink* sink,
                   Com<ITfComposition>& composition, const Reply& reply) {
    HRESULT hr=S_OK;
    if (!reply.commit.empty()) {
        Com<ITfRange> range;
        hr=composition?composition->GetRange(&range):Selection(context,cookie,range);
        if (FAILED(hr)) return hr;
        if (FAILED(hr=range->SetText(cookie,0,reply.commit.data(),static_cast<LONG>(reply.commit.size())))) return hr;
        if (FAILED(hr=MoveCaret(context,cookie,range.Get(),static_cast<int>(reply.commit.size())))) return hr;
        if (FAILED(hr=FinishComposition(cookie,composition,false))) return hr;
    }
    if (reply.preedit.empty()) return FinishComposition(cookie,composition,true);
    if (!composition) {
        Com<ITfRange> range;
        Com<ITfContextComposition> owner;
        if (FAILED(hr=Selection(context,cookie,range)) || FAILED(hr=context->QueryInterface(IID_PPV_ARGS(&owner)))) return hr;
        if (FAILED(hr=owner->StartComposition(cookie,range.Get(),sink,&composition))) return hr;
        if (!composition) return E_FAIL;
    }
    Com<ITfRange> range;
    if (FAILED(hr=composition->GetRange(&range))) return hr;
    if (FAILED(hr=range->SetText(cookie,0,reply.preedit.data(),static_cast<LONG>(reply.preedit.size())))) return hr;
    return MoveCaret(context,cookie,range.Get(),reply.cursor);
}
}
