#pragma once
#include "Composition.h"
#include <UIAutomation.h>
#include <stdexcept>

class PopupAutomation {
public:
    PopupAutomation() {
        if (FAILED(CoCreateInstance(__uuidof(CUIAutomation),nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&automation_))))
            throw std::runtime_error("UI Automation unavailable");
    }
    bool Visible() const { const auto window=FindWindowW(nullptr,L"ZhuyinType 候選字"); return window && IsWindowVisible(window); }
    zt::Com<IUIAutomationElement> Named(const wchar_t* name) {
        const auto root=Root();
        VARIANT value; VariantInit(&value); value.vt=VT_BSTR; value.bstrVal=SysAllocString(name);
        zt::Com<IUIAutomationCondition> condition;
        const auto hr=automation_->CreatePropertyCondition(UIA_NamePropertyId,value,&condition);
        VariantClear(&value);
        if (FAILED(hr)) throw std::runtime_error("UIA condition");
        zt::Com<IUIAutomationElement> element;
        root->FindFirst(TreeScope_Descendants,condition.Get(),&element);
        return element;
    }
    zt::Com<IUIAutomationElement> FirstCandidate() {
        zt::Com<IUIAutomationCondition> all;
        automation_->CreateTrueCondition(&all);
        zt::Com<IUIAutomationElementArray> elements;
        if (FAILED(Root()->FindAll(TreeScope_Descendants,all.Get(),&elements))) throw std::runtime_error("UIA tree");
        int count=0; elements->get_Length(&count);
        for (int i=0;i<count;++i) {
            zt::Com<IUIAutomationElement> element;
            elements->GetElement(i,&element);
            CONTROLTYPEID type=0; element->get_CurrentControlType(&type);
            if (type==UIA_ButtonControlTypeId && Name(element.Get()).starts_with(L"1 ")) return element;
        }
        throw std::runtime_error("accessible first candidate missing");
    }
    static std::wstring Name(IUIAutomationElement* element) {
        BSTR value=nullptr;
        if (!element || FAILED(element->get_CurrentName(&value))) return {};
        std::wstring text=value?value:L""; SysFreeString(value); return text;
    }
    static void Click(IUIAutomationElement* element) {
        RECT rect{}; BOOL enabled=FALSE;
        if (!element || FAILED(element->get_CurrentBoundingRectangle(&rect)) || FAILED(element->get_CurrentIsEnabled(&enabled)) ||
            !enabled || rect.right<=rect.left || rect.bottom<=rect.top) throw std::runtime_error("mouse target unavailable");
        if (!SetCursorPos((rect.left+rect.right)/2,(rect.top+rect.bottom)/2)) throw std::runtime_error("SetCursorPos");
        INPUT events[2]{};
        for (auto& event:events) event.type=INPUT_MOUSE;
        events[0].mi.dwFlags=MOUSEEVENTF_LEFTDOWN; events[1].mi.dwFlags=MOUSEEVENTF_LEFTUP;
        if (SendInput(2,events,sizeof(INPUT))!=2) throw std::runtime_error("mouse SendInput");
    }
private:
    zt::Com<IUIAutomationElement> Root() {
        const auto window=FindWindowW(nullptr,L"ZhuyinType 候選字");
        zt::Com<IUIAutomationElement> root;
        if (!window || !IsWindowVisible(window) || FAILED(automation_->ElementFromHandle(window,&root)))
            throw std::runtime_error("candidate popup missing");
        return root;
    }
    zt::Com<IUIAutomation> automation_;
};
