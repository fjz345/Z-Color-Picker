#include "ZColorPickerBridgeLibrary.h"

#include "Curves/CurveLinearColor.h"
#include "Curves/RichCurve.h"
#include "Factories/CurveFactory.h"
#include "IAssetTools.h"
#include "Misc/PackageName.h"
#include "Modules/ModuleManager.h"
#include "UObject/UObjectGlobals.h"

IMPLEMENT_MODULE(FDefaultModuleImpl, ZColorPickerBridge)

namespace
{
	FString CheckAssetPath(const FString& AssetPath)
	{
		FText Reason;
		if (!FPackageName::IsValidLongPackageName(AssetPath, false, &Reason))
		{
			return FString::Printf(TEXT("Invalid AssetPath '%s': %s"), *AssetPath, *Reason.ToString());
		}
		if (!AssetPath.StartsWith(TEXT("/Game/"), ESearchCase::CaseSensitive))
		{
			return FString::Printf(TEXT("AssetPath '%s' is not under /Game/"), *AssetPath);
		}
		if (!FName(*FPackageName::GetLongPackageAssetName(AssetPath)).IsValidObjectName(Reason))
		{
			return FString::Printf(TEXT("Invalid asset name in '%s': %s"), *AssetPath, *Reason.ToString());
		}
		return FString();
	}

	FString CheckColors(const TArray<FLinearColor>& Colors)
	{
		if (Colors.IsEmpty())
		{
			return TEXT("Colors is empty");
		}
		for (int32 Index = 0; Index < Colors.Num(); ++Index)
		{
			const FLinearColor& Color = Colors[Index];
			if (!FMath::IsFinite(Color.R) || !FMath::IsFinite(Color.G) || !FMath::IsFinite(Color.B) || !FMath::IsFinite(Color.A))
			{
				return FString::Printf(TEXT("Colors[%d] is not finite"), Index);
			}
		}
		return FString();
	}

	UCurveLinearColor* FindOrCreateCurve(const FString& AssetPath, FString& OutError)
	{
		const FString AssetName = FPackageName::GetLongPackageAssetName(AssetPath);
		const FString ObjectPath = AssetPath + TEXT(".") + AssetName;

		UObject* Existing = StaticFindObject(UObject::StaticClass(), nullptr, *ObjectPath);
		if (!Existing && FPackageName::DoesPackageExist(AssetPath))
		{
			LoadPackage(nullptr, *AssetPath, LOAD_None);
			Existing = StaticFindObject(UObject::StaticClass(), nullptr, *ObjectPath);
			if (!Existing)
			{
				OutError = FString::Printf(TEXT("Package '%s' exists but holds no asset named '%s'"), *AssetPath, *AssetName);
				return nullptr;
			}
		}

		if (Existing)
		{
			if (!IsValid(Existing))
			{
				OutError = FString::Printf(TEXT("'%s' is being deleted"), *AssetPath);
				return nullptr;
			}
			UCurveLinearColor* Curve = Cast<UCurveLinearColor>(Existing);
			if (!Curve)
			{
				OutError = FString::Printf(TEXT("'%s' is a %s, not a CurveLinearColor"), *AssetPath, *Existing->GetClass()->GetName());
			}
			return Curve;
		}

		UObject* Created = IAssetTools::Get().CreateAsset(AssetName, FPackageName::GetLongPackagePath(AssetPath), UCurveLinearColor::StaticClass(), NewObject<UCurveLinearColorFactory>());
		UCurveLinearColor* Curve = Cast<UCurveLinearColor>(Created);
		if (!Curve)
		{
			OutError = FString::Printf(TEXT("Could not create a CurveLinearColor at '%s'"), *AssetPath);
		}
		return Curve;
	}

	void WriteKeys(UCurveLinearColor& Curve, const TArray<FLinearColor>& Colors)
	{
		Curve.Modify();
		for (FRichCurve& ChannelCurve : Curve.FloatCurves)
		{
			ChannelCurve.Reset();
		}

		const int32 LastIndex = Colors.Num() - 1;
		for (int32 Index = 0; Index <= LastIndex; ++Index)
		{
			const float Time = LastIndex == 0 ? 0.0f : static_cast<float>(Index) / LastIndex;
			for (int32 Channel = 0; Channel < UE_ARRAY_COUNT(Curve.FloatCurves); ++Channel)
			{
				FRichCurve& ChannelCurve = Curve.FloatCurves[Channel];
				ChannelCurve.SetKeyInterpMode(ChannelCurve.AddKey(Time, Colors[Index].Component(Channel)), RCIM_Linear);
			}
		}

		Curve.OnCurveChanged(Curve.GetCurves());
		Curve.MarkPackageDirty();
	}
}

FString UZColorPickerBridgeLibrary::ApplyGradient(const FString& AssetPath, const TArray<FLinearColor>& Colors)
{
	FString Error = CheckAssetPath(AssetPath);
	if (Error.IsEmpty())
	{
		Error = CheckColors(Colors);
	}
	if (!Error.IsEmpty())
	{
		return Error;
	}

	UCurveLinearColor* Curve = FindOrCreateCurve(AssetPath, Error);
	if (!Curve)
	{
		return Error;
	}
	WriteKeys(*Curve, Colors);
	return FString();
}
